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

fn unique_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "asylum-modpath-{tag}-{}-{:p}",
        std::process::id(),
        &tag
    ));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[test]
fn contained_module_path_accepts_relative_modules() {
    let dir = unique_dir("ok");
    std::fs::write(dir.join("plugin.wasm"), b"x").unwrap();
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub/plugin.wasm"), b"x").unwrap();

    let a = contained_module_path(&dir, "plugin.wasm").unwrap();
    assert!(a.ends_with("plugin.wasm"));
    // `./` and a subdirectory are fine.
    assert!(contained_module_path(&dir, "./plugin.wasm").is_ok());
    assert!(contained_module_path(&dir, "sub/plugin.wasm").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn contained_module_path_rejects_absolute_and_parent() {
    let dir = unique_dir("esc");
    std::fs::write(dir.join("plugin.wasm"), b"x").unwrap();
    // Absolute path.
    assert!(matches!(
        contained_module_path(&dir, "/etc/hosts"),
        Err(Error::Spawn(_))
    ));
    // Parent traversal.
    assert!(matches!(
        contained_module_path(&dir, "../plugin.wasm"),
        Err(Error::Spawn(_))
    ));
    assert!(matches!(
        contained_module_path(&dir, "sub/../../plugin.wasm"),
        Err(Error::Spawn(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn contained_module_path_rejects_symlink_escape() {
    let dir = unique_dir("sym");
    // A secret outside the plugin directory, and a symlink to it inside.
    let outside = unique_dir("secret");
    let secret = outside.join("secret.wasm");
    std::fs::write(&secret, b"top secret").unwrap();
    let link = dir.join("link.wasm");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&secret, &link).unwrap();

    // The symlink resolves outside the plugin root, so it is refused.
    assert!(matches!(
        contained_module_path(&dir, "link.wasm"),
        Err(Error::Spawn(_))
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&outside);
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

#[test]
fn filter_allowed_keeps_only_the_allowlist() {
    let vars = vec![
        ("PATH".to_string(), "/bin".to_string()),
        ("HOME".to_string(), "/home/me".to_string()),
        ("ASYLUM_CONTROL_TOKEN".to_string(), "secret".to_string()),
        ("AWS_SECRET_ACCESS_KEY".to_string(), "shh".to_string()),
        ("GITHUB_TOKEN".to_string(), "ghp_x".to_string()),
    ];
    let kept = filter_allowed(vars.into_iter(), ENV_ALLOWLIST);
    let keys: Vec<&str> = kept.iter().map(|(k, _)| k.as_str()).collect();
    assert!(keys.contains(&"PATH"));
    assert!(keys.contains(&"HOME"));
    assert!(!keys.contains(&"ASYLUM_CONTROL_TOKEN"));
    assert!(!keys.contains(&"AWS_SECRET_ACCESS_KEY"));
    assert!(!keys.contains(&"GITHUB_TOKEN"));
}

#[test]
fn scrubbed_env_drops_non_allowlisted_vars() {
    // Read-only: cargo sets CARGO_* during tests; none may survive scrubbing,
    // and every surviving key must be on the allowlist.
    let env = scrubbed_env();
    for leaked in ["CARGO", "CARGO_PKG_NAME", "CARGO_MANIFEST_DIR"] {
        if std::env::var(leaked).is_ok() {
            assert!(
                !env.iter().any(|(k, _)| k == leaked),
                "{leaked} leaked through scrubbing"
            );
        }
    }
    for (k, _) in &env {
        assert!(
            ENV_ALLOWLIST.contains(&k.as_str()),
            "unexpected env key: {k}"
        );
    }
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
fn invoke_once_timeout_returns_a_prompt_reply() {
    let script = std::env::temp_dir().join(format!("asylum-rttok-{}.sh", std::process::id()));
    std::fs::write(
        &script,
        "read line\necho '{\"id\":1,\"result\":{\"ok\":1}}'\n",
    )
    .unwrap();
    let rt = process_runtime(&format!("sh {}", script.display()));
    let out = invoke_once_timeout(
        &rt,
        std::path::Path::new("."),
        "ping",
        json!({}),
        std::time::Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(out["ok"], 1);
    let _ = std::fs::remove_file(&script);
}

#[cfg(unix)]
#[test]
fn invoke_once_timeout_kills_a_hung_runtime() {
    // Reads the request but never replies; the timeout must fire promptly and
    // not block for the full sleep.
    let script = std::env::temp_dir().join(format!("asylum-rthang-{}.sh", std::process::id()));
    std::fs::write(&script, "read line\nsleep 30\n").unwrap();
    let rt = process_runtime(&format!("sh {}", script.display()));
    let start = std::time::Instant::now();
    let err = invoke_once_timeout(
        &rt,
        std::path::Path::new("."),
        "ping",
        json!({}),
        std::time::Duration::from_millis(300),
    )
    .unwrap_err();
    assert!(matches!(err, Error::Timeout(_)), "got {err:?}");
    assert!(
        start.elapsed() < std::time::Duration::from_secs(10),
        "timeout should not wait for the child to exit"
    );
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
