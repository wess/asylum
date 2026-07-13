use super::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

fn git_ok() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

fn run(dir: &Path, args: &[&str]) {
    Command::new("git").current_dir(dir).args(args).output().unwrap();
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
    let names: Vec<String> = branches(&repo).unwrap().into_iter().map(|b| b.name).collect();
    assert!(names.contains(&"feature".to_string()));
    assert!(names.contains(&"main".to_string()));

    delete(&repo, "feature", true).unwrap();
    let names: Vec<String> = branches(&repo).unwrap().into_iter().map(|b| b.name).collect();
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
    assert!(matches!(outcome, MergeOutcome::FastForward | MergeOutcome::Merged));
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
    assert!(conflicts.iter().any(|p| p.contains("f.txt")), "conflicts: {conflicts:?}");

    match merge(&repo, "feature").unwrap() {
        MergeOutcome::Conflicts(paths) => {
            assert!(paths.iter().any(|p| p.contains("f.txt")), "paths: {paths:?}");
        }
        other => panic!("expected conflicts, got {other:?}"),
    }
    abort_merge(&repo).unwrap();
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
