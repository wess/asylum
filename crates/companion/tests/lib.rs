use super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use store::Db;

#[test]
fn describe_formats_url() {
    assert_eq!(describe("127.0.0.1:8787"), "http://127.0.0.1:8787");
}

#[test]
fn end_to_end_http_request() {
    // Seed a store file the server will open.
    let dir = std::env::temp_dir().join(format!("asylum-companion-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join("store.sqlite");
    {
        let db = Db::open(&db_path).unwrap();
        db.create_project("acme", "/tmp/acme", "main", 1).unwrap();
    }

    // Bind an ephemeral port, read it back, then serve on a thread.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let serve_path = db_path.clone();
    // Serve with a token so we can exercise auth end to end.
    std::thread::spawn(move || {
        let _ = serve_on(listener, serve_path, "s3cret");
    });

    // Without the token, the API is rejected.
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .write_all(b"GET /api/projects HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 401"), "response: {response}");

    // With the bearer token, it succeeds.
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .write_all(b"GET /api/projects HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer s3cret\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");
    assert!(response.contains("acme"), "response: {response}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn responses_are_hardened_and_mutations_need_csrf() {
    let dir = std::env::temp_dir().join(format!("asylum-companion-hdr-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join("store.sqlite");
    let tid = {
        let db = Db::open(&db_path).unwrap();
        let p = db.create_project("acme", "/tmp/acme", "main", 1).unwrap();
        db.create_task(p.id, "Add login", "do it", 1).unwrap().id
    };

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let serve_path = db_path.clone();
    // Empty token: loopback-only mode, where CSRF is the only mutation guard.
    std::thread::spawn(move || {
        let _ = serve_on(listener, serve_path, "");
    });

    // The status page ships a CSP and nosniff, and no wildcard CORS.
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");
    assert!(
        !response.contains("Access-Control-Allow-Origin"),
        "CORS present: {response}"
    );
    assert!(
        response.contains("Content-Security-Policy:"),
        "no CSP: {response}"
    );
    assert!(
        response.contains("X-Content-Type-Options: nosniff"),
        "no nosniff: {response}"
    );

    // A follow-up POST without the CSRF header is refused...
    let mut stream = TcpStream::connect(addr).unwrap();
    let body = r#"{"message":"hi"}"#;
    stream
        .write_all(
            format!(
                "POST /api/tasks/{tid}/followup HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 403"), "response: {response}");

    // ...and accepted with it.
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .write_all(
            format!(
                "POST /api/tasks/{tid}/followup HTTP/1.1\r\nHost: x\r\nX-Asylum-Companion: 1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rate_limiter_caps_within_a_window() {
    use std::time::{Duration, Instant};
    let rl = http::RateLimiter::new(Duration::from_secs(100), 3);
    let t = Instant::now();
    assert!(rl.allow_at(t));
    assert!(rl.allow_at(t));
    assert!(rl.allow_at(t));
    assert!(!rl.allow_at(t), "4th within window is denied");
    // A later window resets the count.
    assert!(rl.allow_at(t + Duration::from_secs(101)));
}

#[test]
fn oversized_and_malformed_requests_get_correct_status() {
    let dir = std::env::temp_dir().join(format!("asylum-companion-lim-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join("store.sqlite");
    {
        let db = Db::open(&db_path).unwrap();
        db.create_project("acme", "/tmp/acme", "main", 1).unwrap();
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let serve_path = db_path.clone();
    std::thread::spawn(move || {
        let _ = serve_on(listener, serve_path, "");
    });

    // A Content-Length far past the body cap is refused with 413.
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .write_all(b"POST /api/tasks/1/followup HTTP/1.1\r\nHost: x\r\nX-Asylum-Companion: 1\r\nContent-Length: 5000000\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 413"), "response: {response}");

    // A non-numeric Content-Length is a 400.
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .write_all(b"POST /api/tasks/1/followup HTTP/1.1\r\nHost: x\r\nContent-Length: abc\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 400"), "response: {response}");

    // The server still answers a normal request afterward (not starved).
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .write_all(b"GET /api/health HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn authorized_requires_matching_bearer_when_token_set() {
    assert!(authorized(None, ""));
    assert!(authorized(Some("Bearer anything"), ""));
    assert!(authorized(Some("Bearer good"), "good"));
    assert!(!authorized(None, "good"));
    assert!(!authorized(Some("Bearer bad"), "good"));
    assert!(!authorized(Some("good"), "good"));
}
