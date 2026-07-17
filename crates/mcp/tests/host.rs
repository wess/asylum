use super::*;
use crate::client::Upstream;
use crate::jsonrpc::{Payload, Response, INVALID_PARAMS, METHOD_NOT_FOUND};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

type Calls = Arc<Mutex<Vec<(String, Value)>>>;

/// An in-memory upstream: canned tool list, records the tool calls it receives,
/// and can be told to refuse `initialize` to model a dead server.
struct Fake {
    tools: Vec<Value>,
    calls: Calls,
    fail_init: bool,
}

impl Fake {
    fn new(tool_names: &[&str]) -> (Box<Fake>, Calls) {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        let tools = tool_names
            .iter()
            .map(|n| json!({ "name": n, "description": "", "inputSchema": { "type": "object" } }))
            .collect();
        (
            Box::new(Fake {
                tools,
                calls: calls.clone(),
                fail_init: false,
            }),
            calls,
        )
    }

    fn dead() -> Box<Fake> {
        Fake {
            tools: vec![],
            calls: Arc::new(Mutex::new(Vec::new())),
            fail_init: true,
        }
        .into()
    }
}

impl Upstream for Fake {
    fn request(&self, method: &str, params: Value) -> Result<Response, String> {
        let reply = match method {
            "initialize" if self.fail_init => {
                return Ok(Response::error(Value::from(0), -1, "no init"))
            }
            "initialize" => json!({ "protocolVersion": "x", "capabilities": {} }),
            "tools/list" => json!({ "tools": self.tools }),
            "resources/list" => json!({ "resources": [] }),
            "prompts/list" => json!({ "prompts": [] }),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or("").to_string();
                self.calls
                    .lock()
                    .unwrap()
                    .push((name.clone(), params["arguments"].clone()));
                json!({ "content": [ { "type": "text", "text": format!("ran {name}") } ] })
            }
            _ => return Ok(Response::error(Value::from(0), METHOD_NOT_FOUND, "no")),
        };
        Ok(Response::result(Value::from(0), reply))
    }
    fn notify(&self, _method: &str, _params: Value) -> Result<(), String> {
        Ok(())
    }
}

fn names(tools: &[Value]) -> Vec<String> {
    tools
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn tools_merge_and_namespace_across_servers() {
    let (gh, _) = Fake::new(&["create_pr"]);
    let (lin, _) = Fake::new(&["create_issue"]);
    let host = Host::new(vec![
        Server::new("github", 0, vec![], vec![], gh),
        Server::new("linear", 0, vec![], vec![], lin),
    ]);
    let mut got = names(&host.tools(0, None));
    got.sort();
    assert_eq!(got, vec!["github__create_pr", "linear__create_issue"]);
}

#[test]
fn call_tool_routes_and_unmangles() {
    let (gh, calls) = Fake::new(&["create_pr"]);
    let host = Host::new(vec![Server::new("github", 0, vec![], vec![], gh)]);
    let payload = host.call_tool(0, "github__create_pr", json!({ "title": "hi" }));
    assert!(matches!(payload, Payload::Result(_)));
    // The upstream saw the *unmangled* name and the original arguments.
    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded, vec![("create_pr".to_string(), json!({ "title": "hi" }))]);
}

#[test]
fn deny_hides_a_tool_and_blocks_calling_it_directly() {
    let (gh, _) = Fake::new(&["safe", "danger"]);
    let host = Host::new(vec![Server::new(
        "github",
        0,
        vec![],
        vec!["danger".into()],
        gh,
    )]);
    assert_eq!(names(&host.tools(0, None)), vec!["github__safe"]);
    // Naming the hidden tool directly is refused, not routed.
    let payload = host.call_tool(0, "github__danger", json!({}));
    match payload {
        Payload::Error { code, .. } => assert_eq!(code, METHOD_NOT_FOUND),
        _ => panic!("expected the denied tool to be rejected"),
    }
}

#[test]
fn unknown_service_and_bad_names_error() {
    let (gh, _) = Fake::new(&["x"]);
    let host = Host::new(vec![Server::new("github", 0, vec![], vec![], gh)]);
    assert!(matches!(
        host.call_tool(0, "nope__x", json!({})),
        Payload::Error { code, .. } if code == METHOD_NOT_FOUND
    ));
    assert!(matches!(
        host.call_tool(0, "not-namespaced", json!({})),
        Payload::Error { code, .. } if code == INVALID_PARAMS
    ));
}

#[test]
fn a_project_scoped_server_shadows_the_global_one() {
    let (global, _) = Fake::new(&["global_tool"]);
    let (scoped, _) = Fake::new(&["scoped_tool"]);
    let host = Host::new(vec![
        Server::new("github", 0, vec![], vec![], global),
        Server::new("github", 5, vec![], vec![], scoped),
    ]);
    // In project 5, the project-scoped server answers for `github`.
    assert_eq!(names(&host.tools(5, None)), vec!["github__scoped_tool"]);
    // In project 0 (and any other project), the global one does.
    assert_eq!(names(&host.tools(0, None)), vec!["github__global_tool"]);
    assert_eq!(names(&host.tools(9, None)), vec!["github__global_tool"]);
}

#[test]
fn only_scopes_the_listing_to_one_service() {
    let (gh, _) = Fake::new(&["a"]);
    let (lin, _) = Fake::new(&["b"]);
    let host = Host::new(vec![
        Server::new("github", 0, vec![], vec![], gh),
        Server::new("linear", 0, vec![], vec![], lin),
    ]);
    assert_eq!(names(&host.tools(0, Some("github"))), vec!["github__a"]);
    assert_eq!(host.services(0), vec!["github", "linear"]);
}

#[test]
fn a_server_that_fails_to_initialize_is_skipped() {
    let (ok, _) = Fake::new(&["good"]);
    let host = Host::new(vec![
        Server::new("github", 0, vec![], vec![], ok),
        Server::new("broken", 0, vec![], vec![], Fake::dead()),
    ]);
    // The healthy server still lists; the dead one contributes nothing.
    assert_eq!(names(&host.tools(0, None)), vec!["github__good"]);
    // Calling into the dead server surfaces an internal error, not a panic.
    assert!(matches!(
        host.call_tool(0, "broken__x", json!({})),
        Payload::Error { .. }
    ));
}
