use super::*;
use crate::{parse, Plugin, TriggerAction};
use std::path::PathBuf;

fn plugin(manifest: &str) -> Plugin {
    parse(manifest, PathBuf::from("/tmp/plugin")).expect("valid manifest")
}

const ACME: &str = r#"
id = "acme"
name = "Acme"
[runtime]
type = "process"
command = "bun run host.ts"
[[trigger]]
on = "run_finished"
invoke = "on_finished"
[[trigger]]
on = "run_finished"
when = "nonzero"
notify = "a run failed"
[[trigger]]
on = "worktree_created"
invoke = "on_worktree"
"#;

#[test]
fn matches_event_only_for_enabled_plugins() {
    let plugins = vec![plugin(ACME)];
    let payload = EventPayload::new("worktree_created");

    // Disabled: nothing fires, even with a matching trigger.
    assert!(fired(&plugins, |_| false, &payload).is_empty());

    // Enabled: the one worktree_created trigger fires.
    let hits = fired(&plugins, |id| id == "acme", &payload);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].plugin.id, "acme");
    assert!(
        matches!(&hits[0].trigger.action, TriggerAction::Invoke { method } if method == "on_worktree")
    );
}

#[test]
fn when_filter_selects_the_failure_trigger() {
    let plugins = vec![plugin(ACME)];

    // A successful finish fires only the unconditional invoke trigger.
    let ok = fired(
        &plugins,
        |_| true,
        &EventPayload::new("run_finished").status("success"),
    );
    assert_eq!(ok.len(), 1);
    assert!(matches!(
        &ok[0].trigger.action,
        TriggerAction::Invoke { .. }
    ));

    // A failure fires both the unconditional invoke and the `when = "nonzero"`
    // notify trigger (nonzero is an alias for failure).
    let bad = fired(
        &plugins,
        |_| true,
        &EventPayload::new("run_finished").status("failure"),
    );
    assert_eq!(bad.len(), 2);
    assert!(bad.iter().any(
        |f| matches!(&f.trigger.action, TriggerAction::Notify { text } if text == "a run failed")
    ));
}

#[test]
fn unmatched_event_fires_nothing() {
    let plugins = vec![plugin(ACME)];
    assert!(fired(&plugins, |_| true, &EventPayload::new("task_merged")).is_empty());
}

#[test]
fn payload_serializes_only_the_fields_that_are_set() {
    let payload = EventPayload::new("run_finished")
        .task(7)
        .run(42)
        .project("/repo")
        .worktree("/repo/.asylum/wt")
        .status("success")
        .code(0);
    let value = serde_json::to_value(&payload).unwrap();
    assert_eq!(value["event"], "run_finished");
    assert_eq!(value["task"], 7);
    assert_eq!(value["run"], 42);
    assert_eq!(value["project"], "/repo");
    assert_eq!(value["worktree"], "/repo/.asylum/wt");
    assert_eq!(value["status"], "success");
    assert_eq!(value["code"], 0);

    // A bare payload omits every absent field rather than emitting nulls.
    let bare = serde_json::to_value(EventPayload::new("task_created")).unwrap();
    let obj = bare.as_object().unwrap();
    assert_eq!(obj.len(), 1);
    assert!(obj.contains_key("event"));
    assert!(!obj.contains_key("run"));
}
