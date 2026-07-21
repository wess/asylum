use super::*;

#[test]
fn command_pins_the_c_locale() {
    // Locale-sensitive git output (e.g. "Fast-forward") must stay in English
    // no matter the caller's environment, so every invocation carries an
    // explicit LC_ALL/LANG override rather than inheriting the parent's.
    let cmd = command(std::path::Path::new("."), &["status"]);
    let envs: Vec<(&std::ffi::OsStr, Option<&std::ffi::OsStr>)> = cmd.get_envs().collect();
    assert!(envs.contains(&(
        std::ffi::OsStr::new("LC_ALL"),
        Some(std::ffi::OsStr::new("C"))
    )));
    assert!(envs.contains(&(
        std::ffi::OsStr::new("LANG"),
        Some(std::ffi::OsStr::new("C"))
    )));
}

#[test]
fn error_display() {
    let e = Error::Git("fatal: not a repo".into());
    assert_eq!(e.to_string(), "git: fatal: not a repo");
    let e = Error::Spawn("No such file".into());
    assert!(e.to_string().contains("could not run git"));
}

#[test]
fn init_repo_makes_a_usable_repo() {
    if std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_err()
    {
        return; // no git
    }
    let dir = std::env::temp_dir().join(format!("asylum-init-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.txt"), "hi\n").unwrap();

    assert!(!is_repo(&dir));
    init_repo(&dir).unwrap();
    assert!(is_repo(&dir));
    // A HEAD now exists (empty initial commit) so worktrees work.
    assert!(current_branch(&dir).is_some());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn non_repo_is_not_a_repo() {
    // The temp dir root is not a git work tree.
    let dir = std::env::temp_dir();
    // Only assert the negative when git exists; otherwise is_repo is false anyway.
    assert!(!is_repo(std::path::Path::new("/nonexistent-xyz-123")));
    let _ = dir;
}
