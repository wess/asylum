use super::*;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from("/plugins/demo")
}

#[test]
fn minimal_manifest() {
    let p = parse("id = \"demo\"\nname = \"Demo\"\n", dir()).unwrap();
    assert_eq!(p.id, "demo");
    assert_eq!(p.name, "Demo");
    assert_eq!(p.version, "0.0.0");
    assert!(p.commands.is_empty());
}

#[test]
fn full_manifest() {
    let text = r#"
id = "linear"
name = "Linear"
version = "1.2.0"
description = "Browse issues"
capabilities = ["network", "notify"]

[runtime]
type = "process"
command = "bun run server.ts"
persistent = true

[panel]
id = "issues"
title = "Issues"
icon = "◪"

[webview]
id = "board"
title = "Board"
placement = "tab"
url = "https://linear.app"

[[command]]
id = "sync"
title = "Sync Issues"
run = "sync"
keybind = "cmd-shift-l"

[[trigger]]
on = "run_finished"
when = "nonzero"
notify = "A run finished"

[[tool]]
id = "create_issue"
description = "Create a Linear issue"
param = [{ name = "title", type = "string", required = true }]
"#;
    let p = parse(text, dir()).unwrap();
    assert_eq!(p.version, "1.2.0");
    assert_eq!(p.capabilities, vec!["network", "notify"]);

    let rt = p.runtime.unwrap();
    assert_eq!(rt.kind, RuntimeKind::Process);
    assert!(rt.persistent);

    assert_eq!(p.panel.unwrap().title, "Issues");

    let wv = p.webview.unwrap();
    assert_eq!(wv.placement, Placement::Tab);
    assert_eq!(wv.source, WebviewSource::Url("https://linear.app".into()));

    assert_eq!(p.commands[0].keybind.as_deref(), Some("cmd-shift-l"));

    let trig = &p.triggers[0];
    assert_eq!(trig.on, "run_finished");
    assert_eq!(trig.action, TriggerAction::Notify { text: "A run finished".into() });

    let tool = &p.tools[0];
    assert_eq!(tool.id, "create_issue");
    assert!(tool.params[0].required);
    assert_eq!(tool.params[0].kind, "string");
}

#[test]
fn rejects_unknown_capability() {
    let text = "id=\"x\"\nname=\"X\"\ncapabilities=[\"telepathy\"]\n";
    let err = parse(text, dir()).unwrap_err();
    assert!(err.contains("telepathy"), "{err}");
}

#[test]
fn rejects_unknown_trigger_event() {
    let text = "id=\"x\"\nname=\"X\"\n[[trigger]]\non=\"nope\"\nnotify=\"hi\"\n";
    let err = parse(text, dir()).unwrap_err();
    assert!(err.contains("nope"), "{err}");
}

#[test]
fn process_runtime_requires_command() {
    let text = "id=\"x\"\nname=\"X\"\n[runtime]\ntype=\"process\"\n";
    assert!(parse(text, dir()).is_err());
}

#[test]
fn webview_requires_a_source() {
    let text = "id=\"x\"\nname=\"X\"\n[webview]\nid=\"w\"\n";
    let err = parse(text, dir()).unwrap_err();
    assert!(err.contains("url, entry, or service"), "{err}");
}

#[test]
fn missing_id_is_error() {
    assert!(parse("name = \"X\"\n", dir()).is_err());
}
