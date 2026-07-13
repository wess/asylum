use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

fn scratch() -> std::path::PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("asylum-checks-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn detects_node_and_rust() {
    let d = scratch();
    std::fs::write(d.join("package.json"), "{}").unwrap();
    std::fs::write(d.join("tsconfig.json"), "{}").unwrap();
    let ids: Vec<String> = detect(&d).into_iter().map(|c| c.id).collect();
    assert!(ids.contains(&"typecheck".to_string()));
    assert!(ids.contains(&"lint".to_string()));
    assert!(ids.contains(&"test".to_string()));

    let d2 = scratch();
    std::fs::write(d2.join("Cargo.toml"), "[package]").unwrap();
    let ids: Vec<String> = detect(&d2).into_iter().map(|c| c.id).collect();
    assert!(ids.contains(&"clippy".to_string()));

    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&d2);
}

#[test]
fn node_without_tsconfig_skips_typecheck() {
    let d = scratch();
    std::fs::write(d.join("package.json"), "{}").unwrap();
    let ids: Vec<String> = detect(&d).into_iter().map(|c| c.id).collect();
    assert!(!ids.contains(&"typecheck".to_string()));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn empty_dir_has_no_checks() {
    let d = scratch();
    assert!(detect(&d).is_empty());
    let _ = std::fs::remove_dir_all(&d);
}

#[cfg(unix)]
#[test]
fn run_classifies_pass_and_fail() {
    let d = scratch();
    let pass = Check {
        id: "ok".into(),
        label: "ok".into(),
        program: "true".into(),
        args: vec![],
    };
    let fail = Check {
        id: "bad".into(),
        label: "bad".into(),
        program: "false".into(),
        args: vec![],
    };
    assert_eq!(run(&d, &pass).status, Status::Pass);
    assert_eq!(run(&d, &fail).status, Status::Fail);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn missing_program_is_skipped() {
    let d = scratch();
    let c = Check {
        id: "x".into(),
        label: "x".into(),
        program: "definitely-not-a-real-binary-xyz".into(),
        args: vec![],
    };
    assert_eq!(run(&d, &c).status, Status::Skipped);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn overall_precedence() {
    let mk = |s: Status| CheckResult {
        id: "x".into(),
        status: s,
        summary: String::new(),
        duration_ms: 0,
    };
    assert_eq!(overall(&[mk(Status::Pass), mk(Status::Fail)]), Status::Fail);
    assert_eq!(overall(&[mk(Status::Pass), mk(Status::Skipped)]), Status::Pass);
    assert_eq!(overall(&[mk(Status::Skipped)]), Status::Skipped);
    assert_eq!(overall(&[]), Status::Skipped);
}
