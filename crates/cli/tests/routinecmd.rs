use super::*;

// The pure halves: what survives a recording, and what the instrumented shell
// is told to do. Spawning a shell and replaying steps are not exercised here.

#[test]
fn a_recording_keeps_the_commands_and_drops_the_noise() {
    let raw = "\
cargo test
# a comment I typed
ls

cargo build --release
clear
exit
";
    assert_eq!(
        clean_steps(raw),
        vec!["cargo test", "cargo build --release"]
    );
}

#[test]
fn a_command_repeated_because_it_failed_is_one_step() {
    // Replaying it twice is at best slow and at worst destructive.
    let raw = "cargo test\ncargo test\ncargo test\ngit push\n";
    assert_eq!(clean_steps(raw), vec!["cargo test", "git push"]);
}

#[test]
fn the_same_command_later_in_the_workflow_is_kept() {
    // Only *consecutive* repeats collapse: running the tests again after a fix
    // is a genuine part of the sequence.
    let raw = "cargo test\ngit commit\ncargo test\n";
    assert_eq!(
        clean_steps(raw),
        vec!["cargo test", "git commit", "cargo test"]
    );
}

#[test]
fn recording_cannot_record_itself() {
    // Otherwise replaying the routine starts a recorder inside it.
    let raw = "asylum routine record release\ncargo test\n";
    assert_eq!(clean_steps(raw), vec!["cargo test"]);
}

#[test]
fn a_session_where_nothing_happened_yields_no_steps() {
    assert!(clean_steps("").is_empty());
    assert!(clean_steps("\n\n  \n").is_empty());
    assert!(clean_steps("ls\npwd\nexit\n").is_empty());
}

#[test]
fn the_recorder_sources_your_own_rc_first() {
    // A shell without your aliases, prompt and PATH changes what the commands
    // mean, and a routine recorded in a stranger's environment is not the one
    // you demonstrated.
    let bash = recorder_rc("/bin/bash", "/tmp/log", "/home/me/.bashrc");
    assert!(bash.contains("source /home/me/.bashrc"));
    let zsh = recorder_rc("/bin/zsh", "/tmp/log", "/home/me/.zshrc");
    assert!(zsh.contains("source /home/me/.zshrc"));
}

#[test]
fn each_shell_gets_the_hook_it_actually_has() {
    // bash reports commands through a DEBUG trap; zsh through preexec. Using
    // one on the other records nothing at all, silently.
    let bash = recorder_rc("/bin/bash", "/tmp/log", "/home/me/.bashrc");
    assert!(bash.contains("DEBUG"));
    assert!(!bash.contains("preexec"));

    let zsh = recorder_rc("/usr/bin/zsh", "/tmp/log", "/home/me/.zshrc");
    assert!(zsh.contains("preexec"));
    assert!(!zsh.contains("DEBUG"));
}

#[test]
fn both_hooks_append_to_the_log_they_were_given() {
    for shell in ["/bin/bash", "/bin/zsh"] {
        let rc = recorder_rc(shell, "/tmp/steps-here", "/home/me/.rc");
        assert!(rc.contains(">> /tmp/steps-here"), "{shell}: {rc}");
    }
}

#[test]
fn the_recorder_says_it_is_recording() {
    // A shell that looks completely ordinary while capturing every command is
    // not something to ship.
    for shell in ["/bin/bash", "/bin/zsh"] {
        let rc = recorder_rc(shell, "/tmp/log", "/home/me/.rc");
        assert!(rc.to_lowercase().contains("recording"), "{shell}");
    }
}
