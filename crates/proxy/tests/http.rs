use super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

/// Drive one request through `handle_connection` and return the raw response.
fn roundtrip(request: &[u8], handler: impl Fn(Request) -> Response + Send + 'static) -> Vec<u8> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        handle_connection(&mut stream, handler);
    });

    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(request).unwrap();
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    let _ = server.join();
    response
}

fn echo(req: Request) -> Response {
    Response {
        status: 200,
        content_type: "application/json".into(),
        body: req.body,
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Conflicting `Content-Length` values are the classic request-smuggling setup:
/// two parsers disagree on where the body ends. Refuse rather than pick one.
#[test]
fn conflicting_content_lengths_are_refused() {
    assert_eq!(parse_content_length("5"), Some(5));
    assert_eq!(parse_content_length(" 5 "), Some(5));
    // A duplicated-but-identical list is unambiguous.
    assert_eq!(parse_content_length("5, 5"), Some(5));
    // A disagreement is not.
    assert_eq!(parse_content_length("5, 6"), None);
    assert_eq!(parse_content_length(""), None);
    assert_eq!(parse_content_length("abc"), None);
    assert_eq!(parse_content_length("-1"), None);
    // No `+5`, no hex, no whitespace-separated pairs.
    assert_eq!(parse_content_length("+5"), None);
    assert_eq!(parse_content_length("0x5"), None);

    // And end to end: two conflicting headers are a 400, not a guess.
    let response = roundtrip(
        b"POST /openai/v1/x HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\nContent-Length: 6\r\n\r\nhello",
        echo,
    );
    assert!(
        text(&response).starts_with("HTTP/1.1 400"),
        "response: {}",
        text(&response)
    );
}

#[test]
fn parses_method_path_bearer_and_body() {
    let response = roundtrip(
        b"POST /openai/v1/chat HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer tok-123\r\nContent-Type: application/json\r\nContent-Length: 7\r\n\r\n{\"a\":1}",
        |req| {
            assert_eq!(req.method, "POST");
            assert_eq!(req.path, "/openai/v1/chat");
            assert_eq!(req.bearer.as_deref(), Some("tok-123"));
            assert_eq!(req.content_type.as_deref(), Some("application/json"));
            assert_eq!(req.body, b"{\"a\":1}");
            Response::text(200, "ok")
        },
    );
    assert!(text(&response).starts_with("HTTP/1.1 200"));
}

/// Header names are case-insensitive on the wire, but the bearer *value* is
/// case-sensitive and must survive lowercasing of the name.
#[test]
fn header_names_are_case_insensitive_but_values_are_preserved() {
    let response = roundtrip(
        b"GET / HTTP/1.1\r\nhost: x\r\nAUTHORIZATION: Bearer MiXeDCaSe\r\ncontent-type: TEXT/Plain\r\n\r\n",
        |req| {
            assert_eq!(req.bearer.as_deref(), Some("MiXeDCaSe"));
            assert_eq!(req.content_type.as_deref(), Some("TEXT/Plain"));
            Response::text(200, "ok")
        },
    );
    assert!(text(&response).starts_with("HTTP/1.1 200"));
}

/// A body larger than the cap must be refused by declared length, without the
/// server first buffering it.
#[test]
fn oversized_declared_body_is_refused() {
    let request = format!(
        "POST /x HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n",
        MAX_BODY_BYTES + 1
    );
    let response = roundtrip(request.as_bytes(), echo);
    assert!(
        text(&response).starts_with("HTTP/1.1 413"),
        "response: {}",
        text(&response)
    );
}

#[test]
fn malformed_request_line_is_a_400() {
    let response = roundtrip(b"GARBAGE\r\n\r\n", echo);
    assert!(
        text(&response).starts_with("HTTP/1.1 400"),
        "response: {}",
        text(&response)
    );
}

/// Bodies are API payloads and may be binary; nothing may assume UTF-8.
#[test]
fn binary_bodies_survive_intact() {
    let payload: Vec<u8> = vec![0x00, 0xff, 0x1b, 0x80, b'\r', b'\n', 0xc3, 0x28];
    let mut request = format!(
        "POST /x HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n",
        payload.len()
    )
    .into_bytes();
    request.extend_from_slice(&payload);

    let response = roundtrip(&request, echo);
    let split = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("headers");
    assert_eq!(&response[split + 4..], &payload[..]);
}

/// The response must pin its length, refuse sniffing, and close - an agent's
/// client should never be able to run the connection on into another request.
#[test]
fn responses_are_hardened() {
    let response = text(&roundtrip(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n", |_| {
        Response::text(200, "hello")
    }));
    assert!(response.contains("X-Content-Type-Options: nosniff"));
    assert!(response.contains("Connection: close"));
    assert!(response.contains("Content-Length: 5"));
}

/// The reason phrase must match the status. A pass-through upstream code like
/// 201 or 404 previously rendered as `HTTP/1.1 404 OK`.
#[test]
fn reason_phrase_matches_the_status() {
    for (status, expect) in [
        (200u16, "HTTP/1.1 200 OK"),
        (201, "HTTP/1.1 201 Created"),
        (204, "HTTP/1.1 204 No Content"),
        (400, "HTTP/1.1 400 Bad Request"),
        (404, "HTTP/1.1 404 Not Found"),
        (429, "HTTP/1.1 429 Too Many Requests"),
        (502, "HTTP/1.1 502 Bad Gateway"),
    ] {
        let response = text(&roundtrip(
            b"GET / HTTP/1.1\r\nHost: x\r\n\r\n",
            move |_| Response {
                status,
                content_type: "text/plain".into(),
                body: b"x".to_vec(),
            },
        ));
        assert!(
            response.starts_with(expect),
            "status {status} rendered as: {}",
            response.lines().next().unwrap_or_default()
        );
    }

    // An unlisted code still gets a class-appropriate phrase, never "OK".
    let response = text(&roundtrip(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n", |_| {
        Response {
            status: 418,
            content_type: "text/plain".into(),
            body: b"x".to_vec(),
        }
    }));
    assert!(
        response.starts_with("HTTP/1.1 418 "),
        "response: {response}"
    );
    assert!(
        !response.starts_with("HTTP/1.1 418 OK"),
        "response: {response}"
    );
}
