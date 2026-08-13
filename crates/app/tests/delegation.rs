use super::*;

// Events in, readable lines out. The point of the screen is that delegation
// stops being invisible, so these are about what a person ends up reading.

fn event(id: i64, kind: &str, run_id: Option<i64>, data: &str) -> Event {
    Event {
        id,
        kind: kind.to_string(),
        task_id: Some(1),
        run_id,
        data: data.to_string(),
        created_at: 1000 + id,
    }
}

/// Run 7 is claude-code, run 8 is codex, anything else is unknown.
fn agents() -> Box<AgentOf> {
    Box::new(|id| match id {
        7 => Some("claude-code".to_string()),
        8 => Some("codex".to_string()),
        _ => None,
    })
}

#[test]
fn a_spawn_reads_as_one_agent_asking_for_another() {
    let events = vec![event(
        1,
        "control_spawn",
        Some(7),
        r#"{"agent":"codex","prompt":"Reproduce the failing test"}"#,
    )];
    let lines = thread(&events, agents().as_ref());
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].who, Who::Agent("claude-code".into()));
    assert_eq!(lines[0].what, "asked codex to Reproduce the failing test");
}

#[test]
fn a_spawn_with_no_prompt_still_says_something() {
    let events = vec![event(1, "control_spawn", Some(7), r#"{"agent":"codex"}"#)];
    let lines = thread(&events, agents().as_ref());
    assert_eq!(lines[0].what, "asked codex for help");
}

#[test]
fn an_event_from_no_run_speaks_as_the_app() {
    // Dispatching and merging are things Asylum did, not an agent.
    let events = vec![event(1, "task_created", None, "")];
    let lines = thread(&events, agents().as_ref());
    assert_eq!(lines[0].who, Who::Ade);
    assert_eq!(lines[0].who.label(), "Asylum");
}

#[test]
fn blocked_is_said_plainly_because_it_is_the_one_that_needs_you() {
    let events = vec![
        event(1, "run_activity", Some(8), r#"{"activity":"working"}"#),
        event(2, "run_activity", Some(8), r#"{"activity":"blocked"}"#),
    ];
    let lines = thread(&events, agents().as_ref());
    assert_eq!(lines[0].what, "is working");
    assert_eq!(lines[1].what, "is blocked and waiting on you");
}

#[test]
fn machinery_is_dropped_rather_than_rendered_as_noise() {
    // A thread that reports every heartbeat is one nobody reads.
    let events = vec![
        event(1, "run_started", Some(7), ""),
        event(2, "some_internal_bookkeeping", Some(7), ""),
        event(3, "run_activity", Some(7), "{}"),
        event(4, "mcp_call", Some(7), "{}"),
    ];
    let lines = thread(&events, agents().as_ref());
    // Only run_started survives: the unknown kind, the activity with no
    // activity, and the tool call with no tool all say nothing.
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].what, "began");
}

#[test]
fn a_long_prompt_is_summarised_not_reprinted() {
    let long = "Fix the flaky deadline test. It has been failing on CI for weeks \
                and nobody has looked at why, so start by reproducing it locally.";
    let events = vec![event(
        1,
        "control_spawn",
        Some(7),
        &serde_json::json!({ "agent": "codex", "prompt": long }).to_string(),
    )];
    let lines = thread(&events, agents().as_ref());
    // The first sentence, and shorter than the prompt — otherwise the thread
    // becomes the thing it was meant to summarise.
    assert!(lines[0].what.contains("Fix the flaky deadline test"));
    assert!(lines[0].what.len() < long.len());
    assert!(lines[0].what.ends_with('…'));
}

#[test]
fn the_thread_keeps_the_order_things_happened_in() {
    let events = vec![
        event(1, "task_created", None, ""),
        event(2, "worktree_created", Some(7), ""),
        event(3, "control_spawn", Some(7), r#"{"agent":"codex"}"#),
        event(4, "run_finished", Some(8), ""),
    ];
    let lines = thread(&events, agents().as_ref());
    let said: Vec<&str> = lines.iter().map(|l| l.who.label()).collect();
    assert_eq!(said, vec!["Asylum", "claude-code", "claude-code", "codex"]);
    assert!(lines.windows(2).all(|w| w[0].at <= w[1].at));
}

#[test]
fn remembering_shows_up_in_the_thread() {
    // It is the only thing an agent does on the control surface that outlives
    // the task, so it must not happen invisibly.
    let lines = thread(
        &[event(
            1,
            "agent_remembered",
            Some(7),
            r#"{"agent":"Reviewer","note":"integration tests need postgres running"}"#,
        )],
        &|_| Some("claude-code".into()),
    );
    assert_eq!(lines.len(), 1);
    assert!(lines[0].what.contains("will remember"));
    assert!(lines[0].what.contains("postgres"));
}
