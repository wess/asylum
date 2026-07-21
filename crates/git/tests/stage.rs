use super::*;
use crate::diff::{against, FileStatus, LineKind};
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
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A repo whose one file has twelve numbered lines, so an edit near the top and
/// another near the bottom land in two separate hunks (the gap exceeds the
/// default context on both sides).
fn tmp_repo() -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("asylum-stage-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    run(&dir, &["init", "-q", "-b", "main"]);
    run(&dir, &["config", "user.email", "t@t.t"]);
    run(&dir, &["config", "user.name", "t"]);
    let body: String = (1..=12).map(|i| format!("line{i}\n")).collect();
    std::fs::write(dir.join("f.txt"), body).unwrap();
    run(&dir, &["add", "."]);
    run(&dir, &["commit", "-q", "-m", "init"]);
    dir
}

/// Rewrite `f.txt` editing line 2 and line 11, producing two hunks against HEAD.
fn edit_two_regions(dir: &Path) {
    let mut lines: Vec<String> = (1..=12).map(|i| format!("line{i}")).collect();
    lines[1] = "line2-edited".to_string();
    lines[10] = "line11-edited".to_string();
    let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(dir.join("f.txt"), body).unwrap();
}

fn changed_contents(files: &[DiffFile]) -> Vec<String> {
    files
        .iter()
        .flat_map(|f| f.hunks.iter())
        .flat_map(|h| h.lines.iter())
        .filter(|l| l.kind != LineKind::Context)
        .map(|l| l.content.clone())
        .collect()
}

#[test]
fn two_far_apart_edits_are_two_hunks() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    edit_two_regions(&repo);
    let files = against(&repo, "HEAD").unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].hunks.len(), 2, "expected two separate hunks");
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn stage_one_of_two_hunks_puts_exactly_it_in_the_index() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    edit_two_regions(&repo);
    let files = against(&repo, "HEAD").unwrap();
    let file = &files[0];

    // Stage only the first hunk (the line-2 edit).
    stage_hunk(&repo, file, &file.hunks[0]).unwrap();

    // The index now carries exactly the line-2 change...
    let staged = staged(&repo).unwrap();
    let staged_lines = changed_contents(&staged);
    assert!(staged_lines.iter().any(|l| l == "line2-edited"));
    assert!(staged_lines.iter().any(|l| l == "line2"));
    assert!(!staged_lines.iter().any(|l| l.contains("line11")));

    // ...and the line-11 change is still only in the worktree.
    let unstaged = unstaged(&repo).unwrap();
    let unstaged_lines = changed_contents(&unstaged);
    assert!(unstaged_lines.iter().any(|l| l == "line11-edited"));
    assert!(!unstaged_lines.iter().any(|l| l.contains("line2")));

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn unstage_restores_the_hunk_to_the_worktree() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    edit_two_regions(&repo);
    let files = against(&repo, "HEAD").unwrap();
    let file = &files[0];

    stage_hunk(&repo, file, &file.hunks[0]).unwrap();
    assert!(!staged(&repo).unwrap().is_empty());

    // Unstage using the currently-staged view of the file.
    let staged_files = staged(&repo).unwrap();
    unstage_hunk(&repo, &staged_files[0], &staged_files[0].hunks[0]).unwrap();

    assert!(staged(&repo).unwrap().is_empty(), "index should be empty");
    // Both edits are back to being merely unstaged worktree changes.
    let unstaged_lines = changed_contents(&unstaged(&repo).unwrap());
    assert!(unstaged_lines.iter().any(|l| l == "line2-edited"));
    assert!(unstaged_lines.iter().any(|l| l == "line11-edited"));

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn stage_both_hunks_sequentially_leaves_a_clean_worktree() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    edit_two_regions(&repo);
    let files = against(&repo, "HEAD").unwrap();
    let file = &files[0];

    // Both patches are built from the same HEAD-based diff; staging the first
    // must not invalidate the second (its region is still at HEAD in the index).
    stage_hunk(&repo, file, &file.hunks[0]).unwrap();
    stage_hunk(&repo, file, &file.hunks[1]).unwrap();

    assert!(unstaged(&repo).unwrap().is_empty(), "nothing left unstaged");
    let staged_lines = changed_contents(&staged(&repo).unwrap());
    assert!(staged_lines.iter().any(|l| l == "line2-edited"));
    assert!(staged_lines.iter().any(|l| l == "line11-edited"));

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn staging_a_hunk_with_context_keeps_the_offsets_correct() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    edit_two_regions(&repo);
    let files = against(&repo, "HEAD").unwrap();
    let file = &files[0];
    // The second hunk starts well into the file; staging it and committing must
    // reproduce exactly the line-11 edit with its surrounding lines intact.
    stage_hunk(&repo, file, &file.hunks[1]).unwrap();
    assert!(commit_staged(&repo, "line 11 only").unwrap());

    let committed = capture(&repo, &["show", "HEAD:f.txt"]);
    assert!(committed.contains("line11-edited"));
    assert!(committed.contains("line10"));
    assert!(committed.contains("line12"));
    // line 2 was not part of the staged hunk, so HEAD still has the original.
    assert!(committed.contains("line2\n"));
    assert!(!committed.contains("line2-edited"));

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn spaced_filename_stages_a_hunk() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    let name = "my file.txt";
    let body: String = (1..=12).map(|i| format!("row{i}\n")).collect();
    std::fs::write(repo.join(name), body).unwrap();
    run(&repo, &["add", "."]);
    run(&repo, &["commit", "-q", "-m", "add spaced"]);

    let mut lines: Vec<String> = (1..=12).map(|i| format!("row{i}")).collect();
    lines[1] = "row2-edited".to_string();
    lines[10] = "row11-edited".to_string();
    let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(repo.join(name), body).unwrap();

    let files = against(&repo, "HEAD").unwrap();
    let file = files.iter().find(|f| f.path == name).unwrap();
    assert_eq!(file.hunks.len(), 2);
    stage_hunk(&repo, file, &file.hunks[0]).unwrap();

    let staged_lines = changed_contents(&staged(&repo).unwrap());
    assert!(staged_lines.iter().any(|l| l == "row2-edited"));
    assert!(!staged_lines.iter().any(|l| l.contains("row11")));

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn added_file_hunk_stages() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    std::fs::write(repo.join("new.txt"), "alpha\nbeta\n").unwrap();
    // Intent-to-add makes the new file show up in `git diff` as an addition
    // without yet staging its content.
    run(&repo, &["add", "-N", "new.txt"]);

    let files = against(&repo, "HEAD").unwrap();
    let file = files.iter().find(|f| f.path == "new.txt").unwrap();
    assert_eq!(file.status, FileStatus::Added);
    stage_hunk(&repo, file, &file.hunks[0]).unwrap();

    let staged = staged(&repo).unwrap();
    let added = staged.iter().find(|f| f.path == "new.txt").unwrap();
    assert_eq!(added.status, FileStatus::Added);
    let staged_lines = changed_contents(std::slice::from_ref(added));
    assert!(staged_lines.iter().any(|l| l == "alpha"));
    assert!(staged_lines.iter().any(|l| l == "beta"));

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn deleted_file_hunk_stages() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    std::fs::remove_file(repo.join("f.txt")).unwrap();

    let files = against(&repo, "HEAD").unwrap();
    let file = files.iter().find(|f| f.path == "f.txt").unwrap();
    assert_eq!(file.status, FileStatus::Deleted);
    stage_hunk(&repo, file, &file.hunks[0]).unwrap();

    let staged = staged(&repo).unwrap();
    let del = staged.iter().find(|f| f.path == "f.txt").unwrap();
    assert_eq!(del.status, FileStatus::Deleted);

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn renamed_file_hunk_is_refused() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    // A rename with a small edit so a hunk exists.
    run(&repo, &["mv", "f.txt", "g.txt"]);
    let mut lines: Vec<String> = (1..=12).map(|i| format!("line{i}")).collect();
    lines[1] = "line2-edited".to_string();
    let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(repo.join("g.txt"), body).unwrap();
    run(&repo, &["add", "-A"]);

    // Read the rename from the staged diff (which detects it), then reset the
    // index so we exercise the refusal against a renamed DiffFile.
    let files = staged(&repo).unwrap();
    let renamed = files
        .iter()
        .find(|f| f.status == FileStatus::Renamed)
        .expect("expected a detected rename");
    let err = stage_hunk(&repo, renamed, &renamed.hunks[0]).unwrap_err();
    assert!(
        err.to_string().contains("renamed"),
        "unexpected error: {err}"
    );

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn stage_file_stages_all_hunks_and_unstage_file_restores() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    edit_two_regions(&repo);
    let files = against(&repo, "HEAD").unwrap();
    let file = &files[0];

    stage_file(&repo, file).unwrap();
    assert!(
        unstaged(&repo).unwrap().is_empty(),
        "whole file should be staged"
    );
    let staged_lines = changed_contents(&staged(&repo).unwrap());
    assert!(staged_lines.iter().any(|l| l == "line2-edited"));
    assert!(staged_lines.iter().any(|l| l == "line11-edited"));

    unstage_file(&repo, file).unwrap();
    assert!(staged(&repo).unwrap().is_empty(), "index should be empty");
    assert_eq!(unstaged(&repo).unwrap()[0].hunks.len(), 2);

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn has_staged_subset_only_when_partial() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    edit_two_regions(&repo);
    let files = against(&repo, "HEAD").unwrap();
    let file = &files[0];

    assert!(
        !has_staged_subset(&repo).unwrap(),
        "nothing staged yet is not a subset"
    );

    stage_hunk(&repo, file, &file.hunks[0]).unwrap();
    assert!(
        has_staged_subset(&repo).unwrap(),
        "one of two hunks staged is a subset"
    );

    stage_hunk(&repo, file, &file.hunks[1]).unwrap();
    assert!(
        !has_staged_subset(&repo).unwrap(),
        "everything staged is not a subset"
    );

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn commit_staged_commits_only_the_staged_hunk() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    edit_two_regions(&repo);
    let files = against(&repo, "HEAD").unwrap();
    let file = &files[0];

    let before: u32 = capture(&repo, &["rev-list", "--count", "HEAD"])
        .trim()
        .parse()
        .unwrap();
    stage_hunk(&repo, file, &file.hunks[0]).unwrap();
    assert!(commit_staged(&repo, "accept line 2").unwrap());
    let after: u32 = capture(&repo, &["rev-list", "--count", "HEAD"])
        .trim()
        .parse()
        .unwrap();
    assert_eq!(after, before + 1);

    // HEAD now has line 2 edited but not line 11...
    let committed = capture(&repo, &["show", "HEAD:f.txt"]);
    assert!(committed.contains("line2-edited"));
    assert!(!committed.contains("line11-edited"));
    // ...and the line-11 edit remains an uncommitted worktree change.
    let unstaged_lines = changed_contents(&unstaged(&repo).unwrap());
    assert!(unstaged_lines.iter().any(|l| l == "line11-edited"));

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn commit_staged_is_a_noop_with_an_empty_index() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    edit_two_regions(&repo);
    // Changes exist in the worktree but nothing is staged.
    assert!(!commit_staged(&repo, "nothing").unwrap());
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn has_worktree_changes_tracks_dirtiness() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    assert!(!has_worktree_changes(&repo).unwrap(), "clean after init");
    edit_two_regions(&repo);
    assert!(has_worktree_changes(&repo).unwrap(), "dirty after edits");
    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn extract_hunk_matches_by_old_start() {
    // Pure unit: two hunks, pick the second by its old-side start line.
    let raw = "diff --git a/f.txt b/f.txt\nindex 111..222 100644\n--- a/f.txt\n+++ b/f.txt\n@@ -2,3 +2,3 @@\n line1\n-line2\n+line2-edited\n@@ -10,3 +10,3 @@\n line9\n-line11\n+line11-edited\n";
    let patch = extract_hunk(raw, 10).expect("hunk at old_start 10");
    assert!(patch.contains("@@ -10,3 +10,3 @@"));
    assert!(patch.contains("line11-edited"));
    assert!(!patch.contains("line2-edited"));
    // Header is preserved so the slice is a self-contained, appliable patch.
    assert!(patch.starts_with("diff --git a/f.txt b/f.txt\n"));
    assert!(patch.contains("--- a/f.txt\n"));
    assert!(extract_hunk(raw, 999).is_none());
}
