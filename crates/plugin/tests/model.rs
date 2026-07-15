use super::*;

#[test]
fn process_runtime_is_trusted_wasm_is_not() {
    assert!(RuntimeKind::Process.is_trusted());
    assert!(!RuntimeKind::Wasm.is_trusted());
}

#[test]
fn trust_summary_discloses_process_command_and_authority() {
    let proc = Runtime {
        kind: RuntimeKind::Process,
        command: "bun run host.ts".into(),
        wasm: None,
        persistent: true,
    };
    let summary = proc.trust_summary();
    assert!(summary.contains("bun run host.ts"), "{summary}");
    assert!(summary.contains("full user privileges"), "{summary}");

    let wasm = Runtime {
        kind: RuntimeKind::Wasm,
        command: String::new(),
        wasm: Some("plugin.wasm".into()),
        persistent: false,
    };
    assert!(wasm.trust_summary().contains("sandboxed"));
}
