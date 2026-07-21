use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

#[test]
fn parses_vimgrep_lines() {
    let out = "src/main.rs:12:5:let x = 1;\nsrc/lib.rs:3:1:fn go() {\n";
    let ms = parse_vimgrep(out);
    assert_eq!(ms.len(), 2);
    assert_eq!(ms[0].file, "src/main.rs");
    assert_eq!(ms[0].line, 12);
    assert_eq!(ms[0].column, 5);
    assert_eq!(ms[0].text, "let x = 1;");
}

#[test]
fn text_may_contain_colons() {
    let out = "a.rs:1:1:let url = \"http://x:8080\";";
    let ms = parse_vimgrep(out);
    assert_eq!(ms[0].text, "let url = \"http://x:8080\";");
}

#[test]
fn ignores_malformed_lines() {
    let out = "garbage without colons\nb.rs:2:1:ok\n";
    let ms = parse_vimgrep(out);
    assert_eq!(ms.len(), 1);
    assert_eq!(ms[0].file, "b.rs");
}

#[test]
fn live_search_finds_seeded_content() {
    // Use git grep against a throwaway repo so the test is backend-independent.
    if std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("asylum-search-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .current_dir(&dir)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "t@t.t"]);
    run(&["config", "user.name", "t"]);
    std::fs::write(dir.join("code.rs"), "fn asylum_marker() {}\n").unwrap();
    run(&["add", "."]);
    run(&["commit", "-qm", "x"]);

    let results = search(&dir, "asylum_marker", &Options::default()).unwrap();
    assert!(
        results.iter().any(|m| m.file.contains("code.rs")),
        "{results:?}"
    );

    // A pattern that matches nothing yields an empty result, not an error.
    let empty = search(&dir, "no_such_symbol_xyz", &Options::default()).unwrap();
    assert!(empty.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn malformed_regex_returns_invalid_pattern() {
    // Use git grep against a throwaway repo so the test is backend-independent.
    if std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("asylum-search-invalid-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .current_dir(&dir)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "t@t.t"]);
    run(&["config", "user.name", "t"]);
    std::fs::write(dir.join("code.rs"), "fn test() {}\n").unwrap();
    run(&["add", "."]);
    run(&["commit", "-qm", "x"]);

    // Unclosed bracket should trigger InvalidPattern error.
    let result = search(&dir, "[", &Options::default());
    match result {
        Err(Error::InvalidPattern(msg)) => {
            // Error message should mention brackets or pattern.
            assert!(!msg.is_empty());
        }
        other => panic!("expected InvalidPattern, got: {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}
