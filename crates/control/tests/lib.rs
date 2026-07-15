use super::*;
use std::net::TcpListener;
use std::time::{SystemTime, UNIX_EPOCH};

use store::Db;

/// A unique temp db path, seeded with one project/task/run, plus its ids.
fn temp_db() -> (std::path::PathBuf, i64, i64) {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("asylum-control-{nanos}.db"));
    let db = Db::open(&path).unwrap();
    let p = db.create_project("R", "/tmp/ctl-e2e", "main", 1).unwrap();
    let t = db.create_task(p.id, "T", "prompt", 1).unwrap();
    let run = db
        .create_run(t.id, "claude-code", "/tmp/wt", "asylum/x")
        .unwrap();
    (path, t.id, run.id)
}

fn start(token: &str) -> (String, std::path::PathBuf, i64, i64) {
    let (path, task, run) = temp_db();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let db_path = path.clone();
    let token = token.to_string();
    std::thread::spawn(move || {
        let _ = serve_on(listener, db_path, token);
    });
    (describe(&addr), path, task, run)
}

#[test]
fn end_to_end_read_and_report_over_a_socket() {
    let (base, path, task, run) = start("");
    let client = Client::new(&base, "").unwrap();

    let (status, body) = client.get("/control/health").unwrap();
    assert_eq!(status, 200);
    assert!(body.contains("\"ok\":true"));

    // Report activity, then read the sibling list back and see it.
    let (status, _) = client
        .post(
            &format!("/control/runs/{run}/activity"),
            r#"{"activity":"working"}"#,
        )
        .unwrap();
    assert_eq!(status, 200);

    let (status, body) = client.get(&format!("/control/runs?task={task}")).unwrap();
    assert_eq!(status, 200);
    assert!(body.contains("\"activity\":\"working\""));

    let _ = std::fs::remove_file(path);
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
    assert!(rl.allow_at(t + Duration::from_secs(101)));
}

#[test]
fn oversized_and_malformed_requests_get_correct_status() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    let (path, _task, _run) = temp_db();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let db_path = path.clone();
    std::thread::spawn(move || {
        let _ = serve_on(listener, db_path, "");
    });

    // A body far past the cap is refused with 413 before it is read.
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(b"POST /control/runs/1/activity HTTP/1.1\r\nHost: x\r\nContent-Length: 5000000\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut resp = String::new();
    s.read_to_string(&mut resp).unwrap();
    assert!(resp.starts_with("HTTP/1.1 413"), "response: {resp}");

    // A non-numeric Content-Length is a 400.
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(b"POST /control/runs/1/activity HTTP/1.1\r\nHost: x\r\nContent-Length: xyz\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut resp = String::new();
    s.read_to_string(&mut resp).unwrap();
    assert!(resp.starts_with("HTTP/1.1 400"), "response: {resp}");

    // Health still answers (the server was not starved).
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(b"GET /control/health HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut resp = String::new();
    s.read_to_string(&mut resp).unwrap();
    assert!(resp.starts_with("HTTP/1.1 200"), "response: {resp}");

    let _ = std::fs::remove_file(path);
}

#[test]
fn a_valid_scoped_token_is_required_except_on_health() {
    // The server's "token" is the signing key; agents present scoped tokens.
    let (base, path, task, run) = start("s3cret");

    // Health is always open.
    let open = Client::new(&base, "").unwrap();
    assert_eq!(open.get("/control/health").unwrap().0, 200);
    // Missing and raw (unsigned) tokens are rejected.
    assert_eq!(
        open.get(&format!("/control/runs?task={task}")).unwrap().0,
        401
    );
    let raw = Client::new(&base, "s3cret").unwrap();
    assert_eq!(
        raw.get(&format!("/control/runs?task={task}")).unwrap().0,
        401
    );

    // A valid token scoped to this task succeeds.
    let scoped = mint("s3cret", task, run, 0);
    let authed = Client::new(&base, &scoped).unwrap();
    assert_eq!(
        authed.get(&format!("/control/runs?task={task}")).unwrap().0,
        200
    );

    // A token scoped to another task cannot reach this one.
    let other = mint("s3cret", task + 999, run, 0);
    let wrong = Client::new(&base, &other).unwrap();
    assert_eq!(
        wrong.get(&format!("/control/runs?task={task}")).unwrap().0,
        403
    );

    let _ = std::fs::remove_file(path);
}
