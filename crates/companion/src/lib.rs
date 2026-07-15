//! The mobile companion server.
//!
//! The mobile companion monitors agents and lets you send follow-ups remotely. This
//! crate is the backend it talks to: a tiny, dependency-light blocking HTTP
//! server over the same SQLite store the desktop app uses. It exposes a JSON API
//! (projects, tasks, runs, notifications, and a follow-up endpoint) plus a
//! mobile-friendly status page at `/`.
//!
//! Routing ([`route`]) is separated from the socket loop ([`serve`]) so the API
//! is testable without a network. The server opens its own connection to the
//! store file, so it runs on a background thread without sharing the desktop
//! app's non-`Sync` `Db`.

mod http;
mod router;

pub use router::{authorized, csrf_ok, route, Response};

use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use store::Db;

/// How many connections may be handled at once. Bounds memory and thread use so
/// a burst of connections cannot spawn unbounded work.
const MAX_CONCURRENT: usize = 8;

/// State-changing requests allowed per [`RATE_WINDOW`] before a `429`.
const RATE_MAX: u32 = 120;
/// Rate-limit window.
const RATE_WINDOW: Duration = Duration::from_secs(10);

/// Shared, read-only server state handed to every worker.
struct Shared {
    db_path: PathBuf,
    token: String,
    limiter: http::RateLimiter,
}

/// Run the companion server on `addr`, serving the store at `db_path`. A
/// non-empty `token` is required as `Authorization: Bearer <token>` on `/api/*`
/// requests. Blocks, one request per connection; intended to run on its own
/// thread.
pub fn serve(
    db_path: impl Into<PathBuf>,
    addr: impl ToSocketAddrs,
    token: impl Into<String>,
) -> std::io::Result<()> {
    serve_on(TcpListener::bind(addr)?, db_path, token)
}

/// Serve on an already-bound listener - lets a caller (or a test) choose the
/// port and read it back before serving.
pub fn serve_on(
    listener: TcpListener,
    db_path: impl Into<PathBuf>,
    token: impl Into<String>,
) -> std::io::Result<()> {
    let shared = Arc::new(Shared {
        db_path: db_path.into(),
        token: token.into(),
        limiter: http::RateLimiter::new(RATE_WINDOW, RATE_MAX),
    });

    // A bounded pool: workers pull accepted connections off a small channel, so
    // handling is concurrent but capped. One slow client can occupy at most one
    // worker (it also carries socket deadlines), never the whole server.
    let (tx, rx) = mpsc::sync_channel::<TcpStream>(MAX_CONCURRENT);
    let rx = Arc::new(Mutex::new(rx));
    for _ in 0..MAX_CONCURRENT {
        let rx = Arc::clone(&rx);
        let shared = Arc::clone(&shared);
        std::thread::spawn(move || loop {
            // Hold the lock only to receive; handling happens unlocked.
            let stream = { rx.lock().unwrap_or_else(|e| e.into_inner()).recv() };
            let Ok(mut stream) = stream else { return };
            serve_one(&mut stream, &shared);
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

/// Handle one accepted connection: open a fresh store connection (keeps store
/// access simple and lock-free across the socket), authenticate, guard against
/// CSRF, rate-limit mutations, then route.
fn serve_one(stream: &mut TcpStream, shared: &Shared) {
    let db = Db::open(&shared.db_path).ok();
    http::handle_connection(stream, |method, path, body, auth, csrf| {
        // Protect the JSON API when a token is configured; the status page and
        // health check stay open so a browser can load them locally.
        if path.starts_with("/api/") && path != "/api/health" && !authorized(auth, &shared.token) {
            return Response::text(401, "unauthorized");
        }
        // Block silent cross-site state changes: a mutation must carry the
        // custom header only our same-origin page can attach.
        if !csrf_ok(method, csrf) {
            return Response::text(403, "forbidden");
        }
        // Rate-limit state-changing requests (follow-ups).
        if is_write_method(method) && !shared.limiter.allow() {
            return Response::text(429, "too many requests");
        }
        match &db {
            Some(db) => route(method, path, body, db),
            None => Response::text(500, "store unavailable"),
        }
    });
}

/// Whether `method` changes state (and so is rate-limited).
fn is_write_method(method: &str) -> bool {
    matches!(method, "POST" | "PUT" | "PATCH" | "DELETE")
}

/// The address the server would bind (host:port), for display.
pub fn describe(addr: &str) -> String {
    format!("http://{addr}")
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
