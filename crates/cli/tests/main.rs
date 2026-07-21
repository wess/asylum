use super::*;

/// The guarantee the task cares about: every entry in the dispatch table -
/// i.e. every subcommand `asylum` actually routes to - has a matching
/// top-level help topic. Add a command to `COMMANDS` without adding a
/// `help::Topic` for it, and this fails.
#[test]
fn every_dispatch_command_has_a_help_topic() {
    for entry in COMMANDS {
        let name = entry.0;
        assert!(
            help::lookup(&[name]).is_some(),
            "dispatcher command `{name}` has no help topic - add one to help::TOPICS"
        );
    }
}

/// The reverse direction: no orphaned top-level help topic for a command
/// that was since removed from the dispatch table.
#[test]
fn every_top_level_help_topic_has_a_dispatch_command() {
    for t in help::TOPICS.iter().filter(|t| t.path.len() == 1) {
        assert!(
            COMMANDS.iter().any(|(name, _)| *name == t.path[0]),
            "help topic `{}` has no matching entry in COMMANDS",
            t.path[0]
        );
    }
}

#[test]
fn command_names_are_unique() {
    for (i, entry) in COMMANDS.iter().enumerate() {
        let name = entry.0;
        assert!(
            COMMANDS[..i].iter().all(|other| other.0 != name),
            "duplicate dispatch entry for `{name}`"
        );
    }
}

#[test]
fn version_and_help_aliases_resolve_to_a_real_command() {
    for raw in ["-V", "--version", "-h", "--help"] {
        let canon = resolve_alias(raw);
        assert!(
            COMMANDS.iter().any(|(name, _)| *name == canon),
            "alias `{raw}` resolves to `{canon}`, which is not in COMMANDS"
        );
    }
    // Anything else passes through unchanged.
    assert_eq!(resolve_alias("worktree"), "worktree");
    assert_eq!(resolve_alias("bogus"), "bogus");
}

#[test]
fn dispatch_reports_unknown_commands_by_name() {
    let err = dispatch("frobnicate", &[]).unwrap_err();
    assert!(err.contains("frobnicate"));
    assert!(err.contains("asylum help"));
}

#[test]
fn dispatch_routes_a_known_command() {
    // `version` never errors and never touches the filesystem or network.
    assert!(dispatch("version", &[]).is_ok());
}

#[test]
fn help_cmd_prints_the_overview_when_bare() {
    assert!(help_cmd(&[]).is_ok());
}

#[test]
fn help_cmd_resolves_a_nested_path() {
    assert!(help_cmd(&["control".to_string(), "status".to_string()]).is_ok());
}

#[test]
fn help_cmd_errors_on_an_unknown_topic() {
    let err = help_cmd(&["bogus".to_string()]).unwrap_err();
    assert!(err.contains("bogus"));
}

#[test]
fn worktree_with_no_subcommand_does_not_panic() {
    // Regression test: this used to slice `&args[1..]` on an empty Vec and
    // panic instead of reporting a usage error.
    let err = worktree(&[]).unwrap_err();
    assert!(err.contains("usage: asylum worktree"));
    assert!(err.contains("worktree --help"));
}

#[test]
fn control_with_no_subcommand_does_not_panic() {
    let err = ctl::control(&[]).unwrap_err();
    assert!(err.contains("usage: asylum control"));
    assert!(err.contains("control --help"));
}
