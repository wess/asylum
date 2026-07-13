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
    std::thread::spawn(move || {
        let _ = serve_on(listener, serve_path);
    });

    // One request.
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .write_all(b"GET /api/projects HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    assert!(response.starts_with("HTTP/1.1 200"), "response: {response}");
    assert!(response.contains("acme"), "response: {response}");

    let _ = std::fs::remove_dir_all(&dir);
}
