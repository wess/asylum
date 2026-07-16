//! The secrets proxy.
//!
//! Lets a running agent make outbound API calls through named **upstreams**
//! without ever seeing the credentials. The agent hits
//! `http://127.0.0.1:<port>/<upstream>/<path>`; the proxy looks up the upstream,
//! resolves the real secret from the encrypted **keep** (scoped to the agent's
//! project - see [`keep`]), injects it server-side, forwards only to that
//! upstream's fixed host, and streams the response back. The agent presents a
//! per-run token that names its project (signed, so it can't be forged); it
//! authorizes *use* of the proxy and *scopes* it, never revealing a secret.
//!
//! Secret values live only in Asylum's memory once the keep is unlocked - never
//! in settings, never in the agent's environment. The proxy only ever *uses* a
//! secret to reach its bound host, and never reflects it back. See [`SKILL`].

mod forward;
mod http;
mod plan;
mod skill;
pub mod token;

pub use plan::{plan, Plan, PlanError};
pub use skill::SKILL;

use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use config::Upstream;
use keep::Keep;

/// Base URL of the secrets proxy, injected into each agent's environment.
pub const ENV_URL: &str = "ASYLUM_PROXY_URL";
/// Per-run signed proxy token (names the agent's project), injected per run.
pub const ENV_TOKEN: &str = "ASYLUM_PROXY_TOKEN";

const MAX_CONCURRENT: usize = 8;
const RATE_MAX: u32 = 240;
const RATE_WINDOW: Duration = Duration::from_secs(10);

/// A live handle to the (possibly still-locked) encrypted keep, shared with the
/// app so unlocking it in the app makes secrets resolvable here.
pub type SharedKeep = Arc<Mutex<Option<Keep>>>;

/// The proxy's live configuration.
pub struct Proxy {
    /// Session signing key used to verify per-run tokens. Empty disables auth
    /// and treats every request as global scope (tests only).
    pub key: String,
    /// Configured upstreams (global and per-project).
    pub upstreams: Vec<Upstream>,
    /// The encrypted keep; `None` inside until unlocked.
    pub keep: SharedKeep,
}

/// Run the secrets proxy on `addr`. Blocks; intended for its own thread. Bind
/// safety (loopback-only) is enforced by the caller.
pub fn serve(addr: impl ToSocketAddrs, proxy: Proxy) -> std::io::Result<()> {
    serve_on(TcpListener::bind(addr)?, proxy)
}

/// Serve on an already-bound listener.
pub fn serve_on(listener: TcpListener, proxy: Proxy) -> std::io::Result<()> {
    let shared = Arc::new(Shared {
        proxy,
        limiter: RateLimiter::new(RATE_WINDOW, RATE_MAX),
    });

    let (tx, rx) = mpsc::sync_channel::<TcpStream>(MAX_CONCURRENT);
    let rx = Arc::new(Mutex::new(rx));
    for _ in 0..MAX_CONCURRENT {
        let rx = Arc::clone(&rx);
        let shared = Arc::clone(&shared);
        std::thread::spawn(move || loop {
            let stream = { rx.lock().unwrap_or_else(|e| e.into_inner()).recv() };
            let Ok(mut stream) = stream else { return };
            http::handle_connection(&mut stream, |req| serve_one(&shared, req));
        });
    }

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if tx.send(stream).is_err() {
            break;
        }
    }
    Ok(())
}

/// The address the server would bind (host:port), for display.
pub fn describe(addr: &str) -> String {
    format!("http://{addr}")
}

struct Shared {
    proxy: Proxy,
    limiter: RateLimiter,
}

/// Authenticate (+ scope), rate-limit, plan, and forward one request.
fn serve_one(shared: &Shared, req: http::Request) -> http::Response {
    if req.path == "/healthz" {
        return http::Response {
            status: 200,
            content_type: "application/json".into(),
            body: b"{\"ok\":true}".to_vec(),
        };
    }

    // The token names the caller's project (signed). Empty key = auth disabled
    // (tests), scope global.
    let project = if shared.proxy.key.is_empty() {
        0
    } else {
        match req
            .bearer
            .as_deref()
            .and_then(|b| token::verify(b, &shared.proxy.key, now()))
        {
            Some(p) => p,
            None => return http::Response::text(401, "unauthorized"),
        }
    };

    // The root lists the upstream names this caller may address (never secrets).
    if req.path == "/" {
        return list_upstreams(&shared.proxy.upstreams, project);
    }
    if !shared.limiter.allow() {
        return http::Response::text(429, "too many requests");
    }

    // Resolve the secret from the keep while briefly holding the lock, then drop
    // it before the (slow) network forward.
    let planned = {
        let guard = shared.proxy.keep.lock().unwrap_or_else(|e| e.into_inner());
        let Some(keep) = guard.as_ref() else {
            return http::Response::text(503, "secrets keep is locked");
        };
        let resolve = |name: &str| keep.resolve(Some(project), name).map(str::to_string);
        plan::plan(&req.path, &shared.proxy.upstreams, project, resolve)
    };

    let plan = match planned {
        Ok(plan) => plan,
        Err(PlanError::UnknownUpstream(_)) | Err(PlanError::BadPath) => {
            return http::Response::text(404, "no such upstream")
        }
        Err(PlanError::MissingSecret(s)) => {
            return http::Response::text(502, &format!("secret not set in keep: {s}"))
        }
        Err(e) => return http::Response::text(502, &format!("upstream misconfigured: {e}")),
    };

    match forward::forward(&plan, &req.method, req.content_type.as_deref(), &req.body) {
        Ok(f) => http::Response {
            status: f.status,
            content_type: f.content_type,
            body: f.body,
        },
        Err(e) => http::Response::text(502, &format!("upstream request failed: {e}")),
    }
}

/// Escape a string for a JSON double-quoted value. Dropping the quotes alone
/// left a name ending in a backslash to escape the closing quote and emit
/// unparseable JSON.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// JSON list of the upstream names visible to `project` (its own plus global).
fn list_upstreams(upstreams: &[Upstream], project: i64) -> http::Response {
    let mut names: Vec<String> = upstreams
        .iter()
        .filter(|u| u.project == project || u.project == 0)
        .map(|u| u.name.clone())
        .collect();
    names.sort();
    names.dedup();
    let json: Vec<String> = names
        .iter()
        .map(|n| format!("\"{}\"", json_escape(n)))
        .collect();
    http::Response {
        status: 200,
        content_type: "application/json".into(),
        body: format!("{{\"upstreams\":[{}]}}", json.join(",")).into_bytes(),
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A fixed-window rate limiter shared across worker threads.
struct RateLimiter {
    window: Duration,
    max: u32,
    state: Mutex<(Instant, u32)>,
}

impl RateLimiter {
    fn new(window: Duration, max: u32) -> Self {
        Self {
            window,
            max,
            state: Mutex::new((Instant::now(), 0)),
        }
    }

    fn allow(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (start, count) = &mut *state;
        if Instant::now().duration_since(*start) > self.window {
            *start = Instant::now();
            *count = 0;
        }
        *count += 1;
        *count <= self.max
    }
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
