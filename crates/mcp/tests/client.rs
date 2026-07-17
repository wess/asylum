use super::*;
use crate::jsonrpc::Payload;
use serde_json::{json, Value};
use std::io::Cursor;

#[test]
fn stdio_request_reads_the_matching_reply() {
    // A server-emitted notification precedes the reply on the wire; the framing
    // must skip it and return the reply carrying our id.
    let wire = concat!(
        r#"{"jsonrpc":"2.0","method":"notifications/message","params":{}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#,
        "\n",
    );
    let mut conn = StdioConn::new(Cursor::new(wire.as_bytes().to_vec()), Vec::<u8>::new());
    let reply = conn.request("tools/list", json!({})).unwrap();
    match reply.payload {
        Payload::Result(v) => assert_eq!(v, json!({ "ok": true })),
        _ => panic!("expected a result"),
    }
}

#[test]
fn stdio_request_errors_when_upstream_closes() {
    let mut conn = StdioConn::new(Cursor::new(Vec::new()), Vec::<u8>::new());
    assert!(conn.request("ping", json!({})).is_err());
}

#[test]
fn stdio_ids_increment_across_requests() {
    // Each request must use a fresh id so a stale reply can't be mismatched to a
    // later request. The second reply (id 2) is only matched if the id advanced.
    let wire = concat!(
        r#"{"jsonrpc":"2.0","id":1,"result":{"n":1}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"result":{"n":2}}"#,
        "\n",
    );
    let mut conn = StdioConn::new(Cursor::new(wire.as_bytes().to_vec()), Vec::<u8>::new());
    let a = conn.request("ping", json!({})).unwrap();
    let b = conn.request("ping", json!({})).unwrap();
    match (a.payload, b.payload) {
        (Payload::Result(x), Payload::Result(y)) => {
            assert_eq!(x["n"], 1);
            assert_eq!(y["n"], 2);
        }
        _ => panic!("expected two results"),
    }
}

#[test]
fn http_response_extracts_session_and_body() {
    let raw = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nMcp-Session-Id: abc123\r\n\r\n\
               {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}";
    let out = parse_http_response(raw);
    assert_eq!(out.session_id.as_deref(), Some("abc123"));
    let v: Value = serde_json::from_str(&out.body).unwrap();
    assert!(v.get("result").is_some());
}

#[test]
fn http_response_unwraps_an_sse_frame() {
    let raw = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n\
               event: message\r\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"x\":1}}\r\n\r\n";
    let out = parse_http_response(raw);
    let v: Value = serde_json::from_str(&out.body).unwrap();
    assert_eq!(v["result"]["x"], 1);
}

#[test]
fn http_response_skips_100_continue() {
    let raw = "HTTP/1.1 100 Continue\r\n\r\n\
               HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":1}";
    let out = parse_http_response(raw);
    assert_eq!(out.body, "{\"ok\":1}");
}

#[test]
fn sse_data_concatenates_data_lines() {
    assert_eq!(sse_data("data: {\"a\"\r\ndata: :1}"), "{\"a\":1}");
}

#[test]
fn curl_escape_neutralizes_quotes_and_newlines() {
    assert_eq!(curl_escape("a\"b"), "a\\\"b");
    assert_eq!(curl_escape("a\nb"), "a\\nb");
    assert_eq!(curl_escape("a\\b"), "a\\\\b");
    assert_eq!(curl_escape("a\rb"), "a\\rb");
}
