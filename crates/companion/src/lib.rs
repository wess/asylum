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

pub use router::{route, Response};

use std::net::{TcpListener, ToSocketAddrs};
use std::path::PathBuf;

use store::Db;

/// Run the companion server on `addr`, serving the store at `db_path`. Blocks,
/// handling one request per connection. Intended to run on its own thread.
pub fn serve(db_path: impl Into<PathBuf>, addr: impl ToSocketAddrs) -> std::io::Result<()> {
    serve_on(TcpListener::bind(addr)?, db_path)
}

/// Serve on an already-bound listener — lets a caller (or a test) choose the
/// port and read it back before serving.
pub fn serve_on(listener: TcpListener, db_path: impl Into<PathBuf>) -> std::io::Result<()> {
    let db_path = db_path.into();
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        // A fresh connection per request keeps store access simple and avoids
        // holding a lock across the socket.
        let db = Db::open(&db_path).ok();
        http::handle_connection(&mut stream, |method, path, body| match &db {
            Some(db) => route(method, path, body, db),
            None => Response::text(500, "store unavailable"),
        });
    }
    Ok(())
}

/// The address the server would bind (host:port), for display.
pub fn describe(addr: &str) -> String {
    format!("http://{addr}")
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
