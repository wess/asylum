use super::*;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

/// Create a throwaway git repo with one commit; returns its path.
fn tmp_repo() -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("asylum-git-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(&dir)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "t@t.t"]);
    run(&["config", "user.name", "t"]);
    std::fs::write(dir.join("readme.md"), "hi\n").unwrap();
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
    dir
}

#[test]
fn parse_list_marks_primary_and_branch() {
    let out = "worktree /repo\nHEAD abc123\nbranch refs/heads/main\n\nworktree /repo/wt/task1\nHEAD def456\nbranch refs/heads/task1\n\n";
    let wts = parse_list(out);
    assert_eq!(wts.len(), 2);
    assert!(wts[0].primary);
    assert_eq!(wts[0].branch.as_deref(), Some("main"));
    assert!(!wts[1].primary);
    assert_eq!(wts[1].path, PathBuf::from("/repo/wt/task1"));
    assert_eq!(wts[1].branch.as_deref(), Some("task1"));
}

#[test]
fn create_list_remove_roundtrip() {
    if Command::new("git").arg("--version").output().is_err() {
        return; // no git available; skip
    }
    let repo = tmp_repo();
    let wt = create(&repo, "wt/task1", Some("task1"), None).unwrap();
    assert!(wt.exists());

    let listed = list(&repo).unwrap();
    assert!(listed.iter().any(|w| w.branch.as_deref() == Some("task1")));

    remove(&repo, &wt, true).unwrap();
    let listed = list(&repo).unwrap();
    assert!(!listed.iter().any(|w| w.branch.as_deref() == Some("task1")));

    let _ = std::fs::remove_dir_all(&repo);
}
