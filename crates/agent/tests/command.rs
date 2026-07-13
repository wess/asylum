use super::*;
use crate::registry::{find, Agent, Delivery};
use config::AgentPrefs;

fn claude() -> Agent {
    find("claude-code").unwrap().to_agent()
}

#[test]
fn arg_delivery_substitutes_token() {
    let spec = build(&claude(), None, "fix the bug", "/wt/a");
    assert_eq!(spec.program, "claude");
    assert_eq!(spec.args, vec!["-p", "fix the bug"]);
    assert_eq!(spec.cwd, "/wt/a");
    assert!(spec.stdin.is_none());
}

#[test]
fn prefs_override_program_and_append_args() {
    let codex = find("codex").unwrap().to_agent();
    let prefs = AgentPrefs {
        program: Some("codex-wrapper".into()),
        extra_args: vec!["--model".into(), "o1".into()],
        enabled: None,
    };
    let spec = build(&codex, Some(&prefs), "do it", "/wt/b");
    assert_eq!(spec.program, "codex-wrapper");
    assert_eq!(spec.args, vec!["exec", "do it", "--model", "o1"]);
}

#[test]
fn stdin_delivery_feeds_stdin_and_drops_token() {
    let def = Agent {
        id: "x".into(),
        name: "X".into(),
        icon: "x".into(),
        program: "xtool".into(),
        args: vec!["--stdin".into(), "{prompt}".into()],
        delivery: Delivery::Stdin,
        builtin: false,
    };
    let spec = build(&def, None, "the prompt", "/wt/c");
    assert_eq!(spec.args, vec!["--stdin"]);
    assert_eq!(spec.stdin.as_deref(), Some("the prompt"));
}

#[test]
fn arg_delivery_without_token_appends_prompt() {
    let def = Agent {
        id: "y".into(),
        name: "Y".into(),
        icon: "y".into(),
        program: "ytool".into(),
        args: vec!["run".into()],
        delivery: Delivery::Arg,
        builtin: false,
    };
    let spec = build(&def, None, "hello", "/wt/d");
    assert_eq!(spec.args, vec!["run", "hello"]);
}

#[test]
fn preview_quotes_whitespace_args() {
    let spec = build(&claude(), None, "two words", "/wt/a");
    assert_eq!(spec.preview(), "claude -p \"two words\"");
}
