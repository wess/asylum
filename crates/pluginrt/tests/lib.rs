use super::*;
use plugin::{Runtime, RuntimeKind};
use serde_json::json;

fn process_runtime(command: &str) -> Runtime {
    Runtime {
        kind: RuntimeKind::Process,
        command: command.to_string(),
        wasm: None,
        persistent: false,
    }
}

#[test]
fn request_response_serde_roundtrip() {
    let req = Request {
        id: 3,
        method: "render".into(),
        params: json!({ "panel": "issues" }),
    };
    let s = serde_json::to_string(&req).unwrap();
    let back: Request = serde_json::from_str(&s).unwrap();
    assert_eq!(back.method, "render");
    assert_eq!(back.params["panel"], "issues");

    let resp: Response = serde_json::from_str(r#"{"id":3,"result":{"ok":true}}"#).unwrap();
    assert_eq!(resp.result.unwrap()["ok"], true);
}

#[test]
fn wasm_runtime_is_unsupported() {
    let rt = Runtime {
        kind: RuntimeKind::Wasm,
        command: String::new(),
        wasm: Some("plugin.wasm".into()),
        persistent: false,
    };
    let err = invoke_once(&rt, std::path::Path::new("."), "x", json!({})).unwrap_err();
    assert!(matches!(err, Error::Unsupported));
}

#[test]
fn spawn_failure_is_reported() {
    let rt = process_runtime("this-binary-does-not-exist-xyz");
    let err = invoke_once(&rt, std::path::Path::new("."), "x", json!({})).unwrap_err();
    assert!(matches!(err, Error::Spawn(_)));
}

#[cfg(unix)]
#[test]
fn one_shot_invoke_reads_response() {
    // A runtime that reads its request then prints one canned response line,
    // preceded by a stray log line the reader must skip. Built as a temp script
    // so the whitespace in the command splits cleanly into `sh <path>`.
    let script = std::env::temp_dir().join(format!("asylum-rt-{}.sh", std::process::id()));
    std::fs::write(
        &script,
        "echo 'starting up'\nread line\necho '{\"id\":1,\"result\":{\"pong\":true}}'\n",
    )
    .unwrap();
    let rt = process_runtime(&format!("sh {}", script.display()));
    let out = invoke_once(&rt, std::path::Path::new("."), "ping", json!({})).unwrap();
    assert_eq!(out["pong"], true);
    let _ = std::fs::remove_file(&script);
}

#[cfg(unix)]
#[test]
fn runtime_error_is_surfaced() {
    let script = std::env::temp_dir().join(format!("asylum-rterr-{}.sh", std::process::id()));
    std::fs::write(&script, "read line\necho '{\"id\":1,\"error\":\"boom\"}'\n").unwrap();
    let rt = process_runtime(&format!("sh {}", script.display()));
    let err = invoke_once(&rt, std::path::Path::new("."), "ping", json!({})).unwrap_err();
    assert!(matches!(err, Error::Runtime(m) if m == "boom"));
    let _ = std::fs::remove_file(&script);
}

#[cfg(unix)]
#[test]
fn persistent_session_handles_multiple_calls() {
    // Echo each request's id back inside a result, forever.
    let script = std::env::temp_dir().join(format!("asylum-rtsess-{}.sh", std::process::id()));
    std::fs::write(
        &script,
        "while read line; do echo '{\"id\":0,\"result\":{\"got\":true}}'; done\n",
    )
    .unwrap();
    let rt = process_runtime(&format!("sh {}", script.display()));
    let mut session = Session::start(&rt, std::path::Path::new(".")).unwrap();
    for _ in 0..3 {
        let out = session.call("tick", json!({})).unwrap();
        assert_eq!(out["got"], true);
    }
    session.shutdown();
    let _ = std::fs::remove_file(&script);
}
