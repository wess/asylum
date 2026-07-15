//! The agent control surface.
//!
//! Asylum races many agents at one task, each isolated in its own worktree. The
//! control surface lets a *running* agent orchestrate that fleet from inside its
//! worktree - the same idea as herdr's agent skill: spawn a helper run, read
//! what a sibling is doing, run the project's checks, report its own semantic
//! state, and wait on another run.
//!
//! It is a small local HTTP/JSON server ([`serve`]) over the same SQLite store
//! the desktop app uses. Reads answer from the store directly; writes that need
//! git/pty effects are queued as [`store::ControlRequest`]s and drained by the
//! app, which keeps [`route`] a pure function tested without sockets. A running
//! agent learns the API from the [`SKILL`] document, and reaches it through the
//! env vars the app injects ([`ENV_URL`], [`ENV_TOKEN`], [`ENV_TASK`],
//! [`ENV_RUN`]).

mod auth;
mod client;
mod http;
mod router;
mod skill;
pub mod token;

pub use client::Client;
pub use router::{route, Response};
pub use skill::SKILL;
pub use token::{mint, verify, Scope};

use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use store::Db;

/// How many connections may be handled at once.
const MAX_CONCURRENT: usize = 8;
/// State-changing control requests allowed per [`RATE_WINDOW`] before a `429`.
const RATE_MAX: u32 = 120;
/// Rate-limit window.
const RATE_WINDOW: Duration = Duration::from_secs(10);

/// Shared, read-only server state handed to every worker.
struct Shared {
    db_path: PathBuf,
    token: String,
    limiter: http::RateLimiter,
}

/// Base URL of the control server, injected into each agent's environment.
pub const ENV_URL: &str = "ASYLUM_CONTROL_URL";
/// Scoped bearer token for the control server: signed with the session key and
/// bound to this run's task, so a run can orchestrate its own fleet but not
/// reach across to another task. Injected per run by the app.
pub const ENV_TOKEN: &str = "ASYLUM_CONTROL_TOKEN";
/// The task id every sibling of a fan-out shares.
pub const ENV_TASK: &str = "ASYLUM_TASK_ID";
/// The agent's own run id. Its presence is how a skill knows it is inside a
/// managed worktree.
pub const ENV_RUN: &str = "ASYLUM_RUN_ID";

/// Run the control server on `addr`, serving the store at `db_path`. `token` is
/// the session *signing key*: when non-empty, every `/control/*` request (except
/// `/control/health`) must carry a valid scoped bearer token signed with it (see
/// [`token`]), and may only touch the task that token was issued for. An empty
/// key disables authentication. Blocks; intended for its own thread.
pub fn serve(
    db_path: impl Into<PathBuf>,
    addr: impl ToSocketAddrs,
    token: impl Into<String>,
) -> std::io::Result<()> {
    serve_on(TcpListener::bind(addr)?, db_path, token)
}

/// Serve on an already-bound listener - lets a caller (or a test) pick the port
/// and read it back first.
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

    // A bounded worker pool: handling is concurrent but capped, and each socket
    // carries deadlines, so one slow or busy client cannot stall the server.
    let (tx, rx) = mpsc::sync_channel::<TcpStream>(MAX_CONCURRENT);
    let rx = Arc::new(Mutex::new(rx));
    for _ in 0..MAX_CONCURRENT {
        let rx = Arc::clone(&rx);
        let shared = Arc::clone(&shared);
        std::thread::spawn(move || loop {
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

/// Handle one accepted connection: authenticate and scope every path except
/// health, rate-limit state-changing requests, then route.
fn serve_one(stream: &mut TcpStream, shared: &Shared) {
    let db = Db::open(&shared.db_path).ok();
    http::handle_connection(stream, |method, path, body, auth| {
        let Some(db) = &db else {
            return Response::text(500, "store unavailable");
        };
        // Health is always open; everything else needs a valid scoped token
        // whose task matches the request's target task.
        if path != "/control/health" {
            match auth::authorize(auth, &shared.token, path, now(), db) {
                Ok(()) => {}
                Err(403) => return Response::text(403, "forbidden"),
                Err(_) => return Response::text(401, "unauthorized"),
            }
        }
        // Rate-limit state-changing requests (spawn / check / activity).
        if is_write_method(method) && !shared.limiter.allow() {
            return Response::text(429, "too many requests");
        }
        route(method, path, body, now(), db)
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

/// Unix seconds.
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
