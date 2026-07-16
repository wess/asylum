use super::*;
use keep::{Keep, Scope};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// A mock upstream that echoes back the `Authorization` header it received, so a
/// test can prove what credential actually reached the upstream.
fn start_mock_upstream() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 1024];
            // Read the head, then drain the declared body so curl gets a clean
            // response rather than a reset.
            let head_end = loop {
                let n = s.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break buf.len();
                }
                buf.extend_from_slice(&chunk[..n]);
                if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break p + 4;
                }
            };
            let head = String::from_utf8_lossy(&buf[..head_end.min(buf.len())]).into_owned();
            let clen: usize = head
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse().unwrap_or(0))
                })
                .unwrap_or(0);
            while buf.len() < head_end + clen {
                let n = s.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            let auth = head
                .lines()
                .find(|l| l.to_ascii_lowercase().starts_with("authorization:"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_string())
                .unwrap_or_default();
            let body = format!("{{\"seen_auth\":\"{auth}\"}}");
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = s.write_all(resp.as_bytes());
            let _ = s.flush();
        }
    });
    format!("http://{addr}")
}

struct Resp {
    status: u16,
    body: String,
}

/// Make a request to the proxy as an agent would.
fn agent_request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    body: &[u8],
) -> Resp {
    let mut s = TcpStream::connect(addr).unwrap();
    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(b) = bearer {
        req.push_str(&format!("Authorization: Bearer {b}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).unwrap();
    s.write_all(body).unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).unwrap();
    let text = String::from_utf8_lossy(&raw).into_owned();
    let status = text
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    Resp { status, body }
}

fn up(name: &str, base: &str, secret: &str, project: i64) -> Upstream {
    Upstream {
        name: name.into(),
        base_url: base.into(),
        secret: secret.into(),
        header: String::new(),
        format: String::new(),
        project,
    }
}

#[test]
fn agent_token_scopes_the_keep_and_upstream_gets_the_real_secret() {
    let base = start_mock_upstream();
    let mut keep = Keep::create("pw").unwrap();
    keep.set(&Scope::Global, "mysecret", "s3cr3t-value");
    keep.set(&Scope::Project(7), "mysecret", "proj7-value");
    let proxy = Proxy {
        key: "sesskey".into(),
        upstreams: vec![up("mock", &base, "mysecret", 0)],
        keep: Arc::new(Mutex::new(Some(keep))),
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let _ = serve_on(listener, proxy);
    });

    // A global-scope token: the upstream receives the global secret, injected by
    // the proxy — a value the agent never sent and never saw.
    let global = token::mint("sesskey", 0, 0);
    let r = agent_request(addr, "POST", "/mock/echo", Some(&global), b"{}");
    assert_eq!(r.status, 200, "body: {}", r.body);
    assert!(
        r.body.contains("Bearer s3cr3t-value"),
        "upstream did not get the injected secret: {}",
        r.body
    );

    // A project-7 token: the SAME upstream now resolves the project-scoped secret
    // (the project keep overlays global) — proving per-project keys.
    let p7 = token::mint("sesskey", 7, 0);
    let r = agent_request(addr, "POST", "/mock/echo", Some(&p7), b"{}");
    assert!(
        r.body.contains("Bearer proj7-value"),
        "project-scoped secret was not used: {}",
        r.body
    );

    // Missing / forged token -> 401. Unknown upstream -> 404. Health open.
    assert_eq!(
        agent_request(addr, "POST", "/mock/echo", None, b"{}").status,
        401
    );
    assert_eq!(
        agent_request(addr, "POST", "/mock/echo", Some("garbage"), b"{}").status,
        401
    );
    assert_eq!(
        agent_request(addr, "GET", "/evil/x", Some(&global), b"").status,
        404
    );
    assert_eq!(
        agent_request(addr, "GET", "/healthz", None, b"").status,
        200
    );
}

#[test]
fn a_locked_keep_refuses_with_503() {
    let proxy = Proxy {
        key: "sesskey".into(),
        upstreams: vec![up("mock", "https://example.com", "s", 0)],
        keep: Arc::new(Mutex::new(None)), // locked
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let _ = serve_on(listener, proxy);
    });
    let tok = token::mint("sesskey", 0, 0);
    assert_eq!(
        agent_request(addr, "GET", "/mock/x", Some(&tok), b"").status,
        503
    );
}
