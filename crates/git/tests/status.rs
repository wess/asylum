use super::*;

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
fn summarize_buckets() {
    let entries = vec![
        Entry { path: "a".into(), kind: StatusKind::Added, staged: true },
        Entry { path: "b".into(), kind: StatusKind::Modified, staged: false },
        Entry { path: "c".into(), kind: StatusKind::Deleted, staged: false },
        Entry { path: "d".into(), kind: StatusKind::Untracked, staged: false },
    ];
    assert_eq!(summarize(&entries), (2, 1, 1));
}
