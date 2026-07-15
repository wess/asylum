use super::*;
use serde_json::json;

/// An echo plugin: logs the params, returns them unchanged. Uses only the
/// always-linked `host_log`.
const ECHO_WAT: &str = r#"
(module
  (import "env" "host_log" (func $host_log (param i32 i32)))
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 2048))
  (func (export "alloc") (param $len i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $len)))
    (local.get $p))
  (func (export "invoke") (param $mp i32) (param $ml i32) (param $pp i32) (param $pl i32) (result i64)
    (call $host_log (local.get $pp) (local.get $pl))
    (i64.or
      (i64.shl (i64.extend_i32_u (local.get $pp)) (i64.const 32))
      (i64.extend_i32_u (local.get $pl)))))
"#;

/// A plugin that imports the gated `host_notify` - only instantiable with the
/// `notify` capability granted.
const NOTIFY_WAT: &str = r#"
(module
  (import "env" "host_notify" (func $host_notify (param i32 i32)))
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 2048))
  (func (export "alloc") (param $len i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (global.get $heap) (local.get $len)))
    (local.get $p))
  (func (export "invoke") (param $mp i32) (param $ml i32) (param $pp i32) (param $pl i32) (result i64)
    (call $host_notify (local.get $pp) (local.get $pl))
    (i64.or
      (i64.shl (i64.extend_i32_u (local.get $pp)) (i64.const 32))
      (i64.extend_i32_u (local.get $pl)))))
"#;

fn compile(wat: &str) -> Vec<u8> {
    wat::parse_str(wat).expect("valid wat")
}

#[test]
fn echo_plugin_roundtrips_json() {
    let bytes = compile(ECHO_WAT);
    let mut rt = WasmRuntime::new(&bytes, &[]).unwrap();
    let params = json!({ "hello": "world", "n": 3 });
    let result = rt.call("ping", &params).unwrap();
    assert_eq!(result, params);
    // The guest logged the params it received.
    assert_eq!(rt.logs().len(), 1);
    assert!(rt.logs()[0].contains("world"));
}

#[test]
fn multiple_calls_accumulate_logs() {
    let bytes = compile(ECHO_WAT);
    let mut rt = WasmRuntime::new(&bytes, &[]).unwrap();
    rt.call("a", &json!({"i":1})).unwrap();
    rt.call("b", &json!({"i":2})).unwrap();
    assert_eq!(rt.logs().len(), 2);
}

#[test]
fn missing_capability_fails_instantiation() {
    let bytes = compile(NOTIFY_WAT);
    // Without the `notify` capability, host_notify is not linked → instantiate
    // fails. That is the capability boundary.
    assert!(WasmRuntime::new(&bytes, &[]).is_err());
    // With it granted, the module instantiates and runs.
    let mut rt = WasmRuntime::new(&bytes, &["notify".to_string()]).unwrap();
    let out = rt.call("x", &json!({"msg":"hi"})).unwrap();
    assert_eq!(out, json!({"msg":"hi"}));
    assert!(rt.logs()[0].starts_with("notify:"));
}

#[test]
fn invoke_wasm_loads_module_from_disk() {
    let bytes = compile(ECHO_WAT);
    let dir = std::env::temp_dir().join(format!("asylum-wasm-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("plugin.wasm"), &bytes).unwrap();

    let runtime = plugin::Runtime {
        kind: plugin::RuntimeKind::Wasm,
        command: String::new(),
        wasm: Some("plugin.wasm".into()),
        persistent: false,
    };
    let out = crate::invoke_wasm(&runtime, &dir, &[], "run", &json!({"ok": true})).unwrap();
    assert_eq!(out, json!({"ok": true}));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rejects_non_wasm_bytes() {
    assert!(WasmRuntime::new(b"not wasm at all", &[]).is_err());
}

/// Spins forever: fuel metering must trap it rather than hang the host.
const INFINITE_LOOP_WAT: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "invoke") (param i32 i32 i32 i32) (result i64)
    (loop $l (br $l))
    (i64.const 0)))
"#;

/// Grows memory forever: the memory limit + trap-on-grow must stop it.
const MEMORY_BOMB_WAT: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "invoke") (param i32 i32 i32 i32) (result i64)
    (loop $l
      (drop (memory.grow (i32.const 1)))
      (br $l))
    (i64.const 0)))
"#;

/// Returns a packed (ptr, len) claiming a 256 MiB result: the host must refuse
/// to allocate for it rather than OOM.
const HUGE_RESULT_WAT: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "invoke") (param i32 i32 i32 i32) (result i64)
    (i64.const 0x10000000)))
"#;

/// Emits 1000 log lines of 1 KiB each (~1 MiB): retained logs must stay capped.
const LOG_FLOOD_WAT: &str = r#"
(module
  (import "env" "host_log" (func $host_log (param i32 i32)))
  (memory (export "memory") 1)
  (func (export "alloc") (param i32) (result i32) (i32.const 0))
  (func (export "invoke") (param i32 i32 i32 i32) (result i64)
    (local $i i32)
    (loop $l
      (call $host_log (i32.const 0) (i32.const 1024))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $l (i32.lt_u (local.get $i) (i32.const 1000))))
    (i64.const 0)))
"#;

#[test]
fn infinite_loop_traps_on_fuel() {
    let bytes = compile(INFINITE_LOOP_WAT);
    let mut rt = WasmRuntime::new(&bytes, &[]).unwrap();
    let err = rt.call("spin", &json!({})).unwrap_err();
    assert!(
        matches!(err, Error::Runtime(_)),
        "expected a trap, got {err:?}"
    );
}

#[test]
fn runaway_memory_growth_is_bounded() {
    let bytes = compile(MEMORY_BOMB_WAT);
    let mut rt = WasmRuntime::new(&bytes, &[]).unwrap();
    let err = rt.call("grow", &json!({})).unwrap_err();
    assert!(
        matches!(err, Error::Runtime(_)),
        "expected a trap, got {err:?}"
    );
}

#[test]
fn oversized_response_is_refused_without_allocating() {
    let bytes = compile(HUGE_RESULT_WAT);
    let mut rt = WasmRuntime::new(&bytes, &[]).unwrap();
    let err = rt.call("big", &json!({})).unwrap_err();
    // A Protocol error (too large), not a crash or OOM.
    assert!(
        matches!(err, Error::Protocol(m) if m.contains("too large")),
        "expected size refusal"
    );
}

#[test]
fn host_log_flood_is_capped() {
    let bytes = compile(LOG_FLOOD_WAT);
    let mut rt = WasmRuntime::new(&bytes, &[]).unwrap();
    rt.call("flood", &json!({})).unwrap();
    let total: usize = rt.logs().iter().map(String::len).sum();
    // Far less than the ~1 MiB the guest tried to log.
    assert!(
        total <= 64 * 1024,
        "retained {total} log bytes, expected <= 64 KiB"
    );
    assert!(!rt.logs().is_empty(), "some logs should still be retained");
}
