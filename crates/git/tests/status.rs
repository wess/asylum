use super::*;
use std::path::PathBuf;
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

fn tmp_repo() -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("asylum-status-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    run(&dir, &["init", "-q", "-b", "main"]);
    run(&dir, &["config", "user.email", "t@t.t"]);
    run(&dir, &["config", "user.name", "t"]);
    std::fs::write(dir.join("readme.md"), "hi\n").unwrap();
    run(&dir, &["add", "."]);
    run(&dir, &["commit", "-q", "-m", "init"]);
    dir
}

#[test]
fn parses_porcelain_v2() {
    // 1: ordinary modified (worktree), 1: staged add, ?: untracked.
    let out = "1 .M N... 100644 100644 100644 aaa bbb src/main.rs\n1 A. N... 000000 100644 100644 000 ccc newfile.rs\n? junk.tmp\n";
    let entries = parse(out);
    assert_eq!(entries.len(), 3);

    assert_eq!(entries[0].path, "src/main.rs");
    assert_eq!(entries[0].kind, StatusKind::Modified);
    assert!(!entries[0].staged);

    assert_eq!(entries[1].path, "newfile.rs");
    assert_eq!(entries[1].kind, StatusKind::Added);
    assert!(entries[1].staged);

    assert_eq!(entries[2].kind, StatusKind::Untracked);
}

#[test]
fn parses_paths_with_spaces() {
    // A staged add and an unstaged modification, each with a space in the
    // filename - the fixed-field count must be used, not a whitespace split.
    let out = "1 A. N... 000000 100644 100644 0000000000000000000000000000000000000000 587be6b4c3f93f93c489c0111bba5596147a26cb new file.txt\n1 .M N... 100644 100644 100644 de980441c3ab03a8c07dda1ad27b8a11f39deb1e de980441c3ab03a8c07dda1ad27b8a11f39deb1e my file.rs\n";
    let entries = parse(out);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].path, "new file.txt");
    assert_eq!(entries[0].kind, StatusKind::Added);
    assert!(entries[0].staged);
    assert_eq!(entries[1].path, "my file.rs");
    assert_eq!(entries[1].kind, StatusKind::Modified);
    assert!(!entries[1].staged);
}

#[test]
fn parses_untracked_path_with_spaces() {
    let out = "? untracked file.txt\n";
    let entries = parse(out);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "untracked file.txt");
    assert_eq!(entries[0].kind, StatusKind::Untracked);
}

#[test]
fn parses_renamed_path_with_spaces() {
    // Both the new and the old path have spaces; porcelain v2 separates them
    // with a tab, not the "orig -> new" arrow of the short format.
    let out = "2 R. N... 100644 100644 100644 422c2b7ab3b3c668038da977e4e93a5fc623169c 422c2b7ab3b3c668038da977e4e93a5fc623169c R100 new name.txt\told name.txt\n";
    let entries = parse(out);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "new name.txt");
    assert_eq!(entries[0].kind, StatusKind::Renamed);
    assert!(entries[0].staged);
}

#[test]
fn parses_unmerged_path_with_spaces() {
    let out = "u UU N... 100644 100644 100644 100644 c0d0fb45c382919737f8d0c20aaf57cf89b74af8 f5630883b18d2741e79039418e5a34a62ad52650 536313deb11ce0e9f885594d8eaaa889f9115b3f merge file.txt\n";
    let entries = parse(out);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "merge file.txt");
    assert_eq!(entries[0].kind, StatusKind::Conflicted);
}

#[test]
fn status_reports_full_paths_with_spaces() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    std::fs::write(repo.join("new file.txt"), "hi\n").unwrap();
    run(&repo, &["add", "new file.txt"]);
    std::fs::write(repo.join("untracked file.txt"), "hi\n").unwrap();

    let entries = status(&repo).unwrap();
    let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    assert!(paths.contains(&"new file.txt"), "paths: {paths:?}");
    assert!(paths.contains(&"untracked file.txt"), "paths: {paths:?}");

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn status_reports_a_rename_with_spaces_in_both_paths() {
    if !git_ok() {
        return;
    }
    let repo = tmp_repo();
    std::fs::write(repo.join("old name.txt"), "content\n").unwrap();
    run(&repo, &["add", "old name.txt"]);
    run(&repo, &["commit", "-q", "-m", "add old name"]);
    run(&repo, &["mv", "old name.txt", "new name.txt"]);

    let entries = status(&repo).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "new name.txt");
    assert_eq!(entries[0].kind, StatusKind::Renamed);
    assert!(entries[0].staged);

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn summarize_buckets() {
    let entries = vec![
        Entry {
            path: "a".into(),
            kind: StatusKind::Added,
            staged: true,
        },
        Entry {
            path: "b".into(),
            kind: StatusKind::Modified,
            staged: false,
        },
        Entry {
            path: "c".into(),
            kind: StatusKind::Deleted,
            staged: false,
        },
        Entry {
            path: "d".into(),
            kind: StatusKind::Untracked,
            staged: false,
        },
    ];
    assert_eq!(summarize(&entries), (2, 1, 1));
}

#[test]
fn managed_worktree_paths_are_excluded() {
    let entries = vec![
        Entry {
            path: ".asylum/worktrees/run/".into(),
            kind: StatusKind::Untracked,
            staged: false,
        },
        Entry {
            path: "src/main.rs".into(),
            kind: StatusKind::Modified,
            staged: false,
        },
    ];
    let remaining = excluding_prefix(entries, Path::new(".asylum/worktrees"));
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].path, "src/main.rs");
}
