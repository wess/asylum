use super::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

fn git_ok() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

fn run(dir: &Path, args: &[&str]) {
    Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
}

fn capture(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap()
}

fn tmp_repo() -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("asylum-branch-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    run(&dir, &["init", "-q", "-b", "main"]);
    run(&dir, &["config", "user.email", "t@t.t"]);
    run(&dir, &["config", "user.name", "t"]);
    std::fs::write(dir.join("f.txt"), "line1\nline2\n").unwrap();
    run(&dir, &["add", "."]);
    run(&dir, &["commit", "-q", "-m", "init"]);
    dir
}

#[test]
fn parse_branches_marks_head() {
    let out = "*\tmain\torigin/main\n \tfeature\t\n";
    let bs = parse_branches(out);
    assert_eq!(bs.len(), 2);
    assert!(bs[0].head);
    assert_eq!(bs[0].name, "main");
    assert_eq!(bs[0].upstream.as_deref(), Some("origin/main"));
    assert!(!bs[1].head);
    assert_eq!(bs[1].upstream, None);
}

#[test]
fn create_list_delete() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    create(&repo, "feature", None).unwrap();
    let names: Vec<String> = branches(&repo)
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert!(names.contains(&"feature".to_string()));
    assert!(names.contains(&"main".to_string()));

    delete(&repo, "feature", true).unwrap();
    let names: Vec<String> = branches(&repo)
        .unwrap()
        .into_iter()
        .map(|b| b.name)
        .collect();
    assert!(!names.contains(&"feature".to_string()));
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn clean_merge_fast_forwards() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    create(&repo, "feature", None).unwrap();
    checkout(&repo, "feature").unwrap();
    std::fs::write(repo.join("g.txt"), "new\n").unwrap();
    run(&repo, &["add", "."]);
    run(&repo, &["commit", "-q", "-m", "add g"]);
    checkout(&repo, "main").unwrap();

    // No conflict expected.
    assert!(would_conflict(&repo, "main", "feature").unwrap().is_empty());
    let outcome = merge(&repo, "feature").unwrap();
    assert!(matches!(
        outcome,
        MergeOutcome::FastForward | MergeOutcome::Merged
    ));
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn conflicting_merge_is_detected_and_reported() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    // Diverge: both branches edit line2 differently.
    create(&repo, "feature", None).unwrap();
    checkout(&repo, "feature").unwrap();
    std::fs::write(repo.join("f.txt"), "line1\nFEATURE\n").unwrap();
    run(&repo, &["commit", "-qam", "feature edit"]);
    checkout(&repo, "main").unwrap();
    std::fs::write(repo.join("f.txt"), "line1\nMAIN\n").unwrap();
    run(&repo, &["commit", "-qam", "main edit"]);

    let conflicts = would_conflict(&repo, "main", "feature").unwrap();
    assert!(
        conflicts.iter().any(|p| p.contains("f.txt")),
        "conflicts: {conflicts:?}"
    );

    match merge(&repo, "feature").unwrap() {
        MergeOutcome::Conflicts(paths) => {
            assert!(
                paths.iter().any(|p| p.contains("f.txt")),
                "paths: {paths:?}"
            );
        }
        other => panic!("expected conflicts, got {other:?}"),
    }
    abort_merge(&repo).unwrap();
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn commit_all_captures_uncommitted_run_changes() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    std::fs::write(repo.join("new.txt"), "result\n").unwrap();
    assert!(commit_all(&repo, "Complete task").unwrap());
    assert!(crate::status::status(&repo).unwrap().is_empty());
    assert!(!commit_all(&repo, "Nothing to do").unwrap());
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn merge_base_of_diverged_branches() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    create(&repo, "feature", None).unwrap();
    let base = merge_base(&repo, "main", "feature").unwrap();
    assert!(base.is_some());
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn clean_squash_merge_makes_one_commit_with_the_default_message() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    create(&repo, "feature", None).unwrap();
    checkout(&repo, "feature").unwrap();
    std::fs::write(repo.join("g.txt"), "new\n").unwrap();
    run(&repo, &["add", "."]);
    run(&repo, &["commit", "-q", "-m", "add g"]);
    std::fs::write(repo.join("h.txt"), "newer\n").unwrap();
    run(&repo, &["add", "."]);
    run(&repo, &["commit", "-q", "-m", "add h"]);
    checkout(&repo, "main").unwrap();

    // Feature is strictly ahead of main, so git's own --squash output reports
    // "Fast-forward" even though nothing is committed yet - the outcome must
    // still be a real, separate commit, not a no-op.
    let before: u32 = capture(&repo, &["rev-list", "--count", "HEAD"])
        .trim()
        .parse()
        .unwrap();
    let outcome = merge_squash(&repo, "feature", None).unwrap();
    assert_eq!(outcome, MergeOutcome::Merged);

    let after: u32 = capture(&repo, &["rev-list", "--count", "HEAD"])
        .trim()
        .parse()
        .unwrap();
    assert_eq!(after, before + 1, "squash should add exactly one commit");
    assert_eq!(
        capture(&repo, &["log", "-1", "--format=%s"]).trim(),
        "Squash feature"
    );
    // A single parent - a plain commit, not a two-parent merge commit.
    assert_eq!(
        capture(&repo, &["log", "-1", "--format=%P"])
            .trim()
            .split(' ')
            .filter(|s| !s.is_empty())
            .count(),
        1
    );
    assert!(repo.join("g.txt").exists());
    assert!(repo.join("h.txt").exists());
    assert!(crate::status::status(&repo).unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn squash_merge_accepts_a_custom_message() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    create(&repo, "feature", None).unwrap();
    checkout(&repo, "feature").unwrap();
    std::fs::write(repo.join("g.txt"), "new\n").unwrap();
    run(&repo, &["add", "."]);
    run(&repo, &["commit", "-q", "-m", "add g"]);
    checkout(&repo, "main").unwrap();

    let outcome = merge_squash(&repo, "feature", Some("Custom squash message")).unwrap();
    assert_eq!(outcome, MergeOutcome::Merged);
    assert_eq!(
        capture(&repo, &["log", "-1", "--format=%s"]).trim(),
        "Custom squash message"
    );
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn squash_merge_is_up_to_date_when_branch_has_nothing_new() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    create(&repo, "feature", None).unwrap();
    // "feature" is exactly HEAD - nothing to squash, nothing to commit.
    let outcome = merge_squash(&repo, "feature", None).unwrap();
    assert_eq!(outcome, MergeOutcome::UpToDate);
    assert!(crate::status::status(&repo).unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn conflicting_squash_merge_is_detected_and_fully_recoverable() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    create(&repo, "feature", None).unwrap();
    checkout(&repo, "feature").unwrap();
    std::fs::write(repo.join("f.txt"), "line1\nFEATURE\n").unwrap();
    run(&repo, &["commit", "-qam", "feature edit"]);
    checkout(&repo, "main").unwrap();
    std::fs::write(repo.join("f.txt"), "line1\nMAIN\n").unwrap();
    run(&repo, &["commit", "-qam", "main edit"]);

    let before_head = capture(&repo, &["rev-parse", "HEAD"]);

    match merge_squash(&repo, "feature", None).unwrap() {
        MergeOutcome::Conflicts(paths) => {
            assert!(
                paths.iter().any(|p| p.contains("f.txt")),
                "paths: {paths:?}"
            );
        }
        other => panic!("expected conflicts, got {other:?}"),
    }

    // A squash merge records no MERGE_HEAD, so the ordinary abort refuses.
    assert!(abort_merge(&repo).is_err());

    // The squash-specific recovery fully restores a clean base.
    abort_squash_merge(&repo).unwrap();
    assert!(crate::status::status(&repo).unwrap().is_empty());
    assert_eq!(capture(&repo, &["rev-parse", "HEAD"]), before_head);
    assert_eq!(
        std::fs::read_to_string(repo.join("f.txt")).unwrap(),
        "line1\nMAIN\n"
    );
    let _ = std::fs::remove_dir_all(&repo);
}
