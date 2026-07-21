use super::*;

#[test]
fn lookup_finds_top_level_and_nested_topics() {
    assert!(lookup(&["worktree"]).is_some());
    assert!(lookup(&["worktree", "create"]).is_some());
    assert!(lookup(&["control", "status"]).is_some());
    assert!(lookup(&["mcp", "serve"]).is_some());
    assert!(lookup(&["keep", "set"]).is_some());
    assert!(lookup(&["plugin", "install"]).is_some());
}

#[test]
fn lookup_truncates_extra_positionals_instead_of_failing() {
    // A trailing positional (a run id, a layout name, ...) shouldn't break
    // the lookup used by `for_invocation`.
    let t = lookup(&["control", "read", "42"]).expect("truncates to [control, read]");
    assert_eq!(t.path, &["control", "read"]);
}

#[test]
fn lookup_rejects_unknown_topics() {
    assert!(lookup(&["bogus"]).is_none());
    assert!(lookup(&["worktree", "bogus"]).is_none());
}

#[test]
fn nested_topics_have_a_registered_parent() {
    for t in TOPICS.iter().filter(|t| t.path.len() == 2) {
        assert!(
            lookup(&[t.path[0]]).is_some(),
            "nested topic `{:?}` has no top-level parent topic",
            t.path
        );
    }
}

#[test]
fn every_top_level_topic_has_a_group() {
    for t in TOPICS.iter().filter(|t| t.path.len() == 1) {
        assert!(
            t.group.is_some(),
            "top-level topic `{}` has no overview group",
            t.path[0]
        );
        assert!(
            GROUPS.contains(&t.group.unwrap()),
            "top-level topic `{}` has an unknown group `{:?}`",
            t.path[0],
            t.group
        );
    }
}

#[test]
fn every_topic_has_usage_and_a_summary() {
    for t in TOPICS {
        assert!(!t.summary.is_empty(), "topic `{:?}` has no summary", t.path);
        assert!(
            !t.usage.is_empty(),
            "topic `{:?}` has no usage line",
            t.path
        );
        assert!(
            !t.examples.is_empty(),
            "topic `{:?}` has no example",
            t.path
        );
    }
}

#[test]
fn render_includes_usage_args_and_examples() {
    let t = lookup(&["worktree", "create"]).unwrap();
    let text = render(t);
    assert!(text.starts_with("asylum worktree create - create a new git worktree"));
    assert!(text.contains("USAGE:"));
    assert!(text.contains("asylum worktree create <path>"));
    assert!(text.contains("ARGS:"));
    assert!(text.contains("--branch <name>"));
    assert!(text.contains("EXAMPLES:"));
}

#[test]
fn render_omits_empty_sections() {
    let t = lookup(&["mcp", "list"]).unwrap();
    let text = render(t);
    assert!(!text.contains("ARGS:"));
    assert!(!text.contains("NOTES:"));
    assert!(text.contains("EXAMPLES:"));
}

#[test]
fn for_invocation_prefers_a_documented_nested_subcommand() {
    let rest = vec!["create".to_string(), "../x".to_string()];
    let t = for_invocation("worktree", &rest).unwrap();
    assert_eq!(t.path, &["worktree", "create"]);
}

#[test]
fn for_invocation_falls_back_to_the_top_level_topic() {
    // "codex" isn't a documented nested subcommand of `run`.
    let rest = vec!["codex".to_string()];
    let t = for_invocation("run", &rest).unwrap();
    assert_eq!(t.path, &["run"]);
}

#[test]
fn for_invocation_ignores_a_flag_like_first_token() {
    let rest = vec!["--help".to_string()];
    let t = for_invocation("worktree", &rest).unwrap();
    assert_eq!(t.path, &["worktree"]);
}

#[test]
fn for_invocation_is_none_for_an_unknown_command() {
    let rest: Vec<String> = vec![];
    assert!(for_invocation("bogus", &rest).is_none());
}

#[test]
fn hint_points_at_the_full_path() {
    assert_eq!(
        hint(&["worktree", "create"]),
        "(see `asylum worktree create --help`)"
    );
    assert_eq!(hint(&["run"]), "(see `asylum run --help`)");
}

#[test]
fn overview_lists_every_top_level_command_under_its_group() {
    let text = overview();
    for t in TOPICS.iter().filter(|t| t.path.len() == 1) {
        assert!(
            text.contains(t.path[0]),
            "overview is missing top-level command `{}`",
            t.path[0]
        );
    }
    // Spot-check a couple of the required theme groupings.
    assert!(text.to_uppercase().contains("FLEET CONTROL"));
    assert!(text.to_uppercase().contains("COMPUTER USE"));
}
