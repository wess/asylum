use super::*;
use crate::client::Upstream;
use crate::jsonrpc::{Payload, Request, Response};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

// --- a minimal in-memory upstream for driving the handler -------------------

struct Fake {
    tools: Vec<Value>,
    calls: Arc<Mutex<Vec<String>>>,
}

fn fake(tool_names: &[&str]) -> (Box<Fake>, Arc<Mutex<Vec<String>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let tools = tool_names
        .iter()
        .map(|n| json!({ "name": n, "description": format!("tool {n}"), "inputSchema": {} }))
        .collect();
    (
        Box::new(Fake {
            tools,
            calls: calls.clone(),
        }),
        calls,
    )
}

impl Upstream for Fake {
    fn request(&self, method: &str, params: Value) -> Result<Response, String> {
        let reply = match method {
            "initialize" => json!({ "protocolVersion": "x", "capabilities": {} }),
            "tools/list" => json!({ "tools": self.tools }),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or("").to_string();
                self.calls.lock().unwrap().push(name.clone());
                json!({ "content": [ { "type": "text", "text": format!("ran {name}") } ] })
            }
            _ => return Ok(Response::error(Value::from(0), -32601, "no")),
        };
        Ok(Response::result(Value::from(0), reply))
    }
    fn notify(&self, _method: &str, _params: Value) -> Result<(), String> {
        Ok(())
    }
}

fn gateway(expose: Expose) -> (Gateway, Arc<Mutex<Vec<String>>>) {
    let (gh, calls) = fake(&["create_pr"]);
    let host = Host::new(vec![Server::new("github", 0, vec![], vec![], gh)]);
    (Gateway::new(host, expose), calls)
}

fn req(method: &str, params: Value) -> Request {
    Request {
        id: Some(json!(1)),
        method: method.into(),
        params,
    }
}

fn result_of(response: Option<Response>) -> Value {
    match response.expect("a response was owed").payload {
        Payload::Result(v) => v,
        Payload::Error { message, .. } => panic!("unexpected error: {message}"),
    }
}

// --- handler behavior -------------------------------------------------------

#[test]
fn initialize_reports_protocol_and_identity() {
    let (gw, _) = gateway(Expose::Direct);
    let v = result_of(handle(&gw, 0, 0, None, req("initialize", json!({}))));
    assert_eq!(v["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(v["serverInfo"]["name"], SERVER_NAME);
    assert!(v["capabilities"]["tools"].is_object());
}

#[test]
fn a_notification_owes_no_response() {
    let (gw, _) = gateway(Expose::Direct);
    let note = Request {
        id: None,
        method: "notifications/initialized".into(),
        params: Value::Null,
    };
    assert!(handle(&gw, 0, 0, None, note).is_none());
}

#[test]
fn an_unknown_method_is_method_not_found() {
    let (gw, _) = gateway(Expose::Direct);
    match handle(&gw, 0, 0, None, req("does/notexist", json!({}))).unwrap().payload {
        Payload::Error { code, .. } => assert_eq!(code, jsonrpc::METHOD_NOT_FOUND),
        _ => panic!("expected method-not-found"),
    }
}

#[test]
fn direct_mode_lists_namespaced_tools() {
    let (gw, _) = gateway(Expose::Direct);
    let v = result_of(handle(&gw, 0, 0, None, req("tools/list", json!({}))));
    let names: Vec<&str> = v["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["github__create_pr"]);
}

#[test]
fn search_mode_lists_only_the_meta_tools() {
    let (gw, _) = gateway(Expose::Search);
    let v = result_of(handle(&gw, 0, 0, None, req("tools/list", json!({}))));
    let names: Vec<&str> = v["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec![catalog::FIND_TOOL, catalog::CALL_TOOL]);
}

#[test]
fn direct_mode_routes_a_tool_call() {
    let (gw, calls) = gateway(Expose::Direct);
    let v = result_of(handle(
        &gw,
        0,
        0,
        None,
        req("tools/call", json!({ "name": "github__create_pr", "arguments": { "x": 1 } })),
    ));
    assert!(v["content"][0]["text"].as_str().unwrap().contains("create_pr"));
    assert_eq!(*calls.lock().unwrap(), vec!["create_pr".to_string()]);
}

#[test]
fn search_mode_find_then_call() {
    let (gw, calls) = gateway(Expose::Search);
    // find returns a text block of matches.
    let found = result_of(handle(
        &gw,
        0,
        0,
        None,
        req("tools/call", json!({ "name": catalog::FIND_TOOL, "arguments": { "query": "pr" } })),
    ));
    let text = found["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("github__create_pr"));

    // call routes through to the underlying tool.
    result_of(handle(
        &gw,
        0,
        0,
        None,
        req(
            "tools/call",
            json!({ "name": catalog::CALL_TOOL, "arguments": { "name": "github__create_pr", "arguments": {} } }),
        ),
    ));
    assert_eq!(*calls.lock().unwrap(), vec!["create_pr".to_string()]);
}

#[test]
fn a_tool_call_is_audited_with_the_run() {
    let (gh, _) = fake(&["create_pr"]);
    let host = Host::new(vec![Server::new("github", 0, vec![], vec![], gh)]);
    let seen: Arc<Mutex<Vec<Audit>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let gw = Gateway {
        key: String::new(),
        host,
        expose: Expose::Direct,
        audit: Some(Box::new(move |a| sink.lock().unwrap().push(a))),
    };
    handle(
        &gw,
        3,
        99,
        None,
        req("tools/call", json!({ "name": "github__create_pr", "arguments": {} })),
    );
    let recorded = seen.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].run, 99);
    assert_eq!(recorded[0].project, 3);
    assert_eq!(recorded[0].tool, "github__create_pr");
    assert!(recorded[0].ok);
}

#[test]
fn resources_read_requires_a_uri() {
    let (gw, _) = gateway(Expose::Direct);
    match handle(&gw, 0, 0, None, req("resources/read", json!({}))).unwrap().payload {
        Payload::Error { code, .. } => assert_eq!(code, jsonrpc::INVALID_PARAMS),
        _ => panic!("expected invalid-params"),
    }
}

// --- socket + auth end to end -----------------------------------------------

fn http_send(addr: SocketAddr, raw: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(raw.as_bytes()).unwrap();
    let mut buf = String::new();
    stream.read_to_string(&mut buf).unwrap();
    let (head, body) = buf.split_once("\r\n\r\n").unwrap_or((&buf, ""));
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .unwrap();
    (status, body.to_string())
}

fn post(addr: SocketAddr, path: &str, token: Option<&str>, body: &str) -> (u16, String) {
    let mut raw = format!(
        "POST {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n",
        body.len()
    );
    if let Some(t) = token {
        raw.push_str(&format!("Authorization: Bearer {t}\r\n"));
    }
    raw.push_str("\r\n");
    raw.push_str(body);
    http_send(addr, &raw)
}

#[test]
fn serves_over_a_socket_and_enforces_auth() {
    let (gh, _) = fake(&["create_pr"]);
    let host = Host::new(vec![Server::new("github", 0, vec![], vec![], gh)]);
    let key = "session-key";
    let gw = Gateway {
        key: key.into(),
        host,
        expose: Expose::Direct,
        audit: None,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let _ = serve_on(listener, gw);
    });

    // Health is open.
    let (status, _) = http_send(
        addr,
        "GET /healthz HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
    );
    assert_eq!(status, 200);

    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
    // No token → 401.
    assert_eq!(post(addr, "/mcp", None, init).0, 401);
    // A bad token → 401.
    assert_eq!(post(addr, "/mcp", Some("garbage"), init).0, 401);

    // A valid token → 200 with the initialize result.
    let good = token::mint(key, 0, 0, 0);
    let (status, body) = post(addr, "/mcp", Some(&good), init);
    assert_eq!(status, 200);
    assert!(body.contains("protocolVersion"));

    // A GET on the MCP endpoint (SSE open) is not offered in this subset.
    let (status, _) = http_send(
        addr,
        "GET /mcp HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
    );
    assert_eq!(status, 405);
}
