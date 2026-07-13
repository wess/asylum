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
    let argv = Host::new("box").port(2222).identity("/keys/id").exec("true");
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
    let create = Host::new("box").worktree_create("~/repo", "wt/task", Some("task"));
    let cmd = create.last().unwrap();
    assert!(cmd.contains("cd ~/repo &&"));
    assert!(cmd.contains("git worktree add -b task wt/task HEAD"));

    let remove = Host::new("box").worktree_remove("~/repo", "wt/task");
    assert!(remove.last().unwrap().contains("git worktree remove --force wt/task"));
}

#[test]
fn keepalive_zero_omits_option() {
    let mut h = Host::new("box");
    h.keepalive_secs = 0;
    let argv = h.exec("true");
    assert!(!argv.iter().any(|a| a.starts_with("ServerAliveInterval")));
}
