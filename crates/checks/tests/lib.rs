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
    std::fs::write(
        d.join("package.json"),
        r#"{"scripts":{"typecheck":"tsc --noEmit","lint":"eslint .","test":"bun test"}}"#,
    )
    .unwrap();
    std::fs::write(d.join("tsconfig.json"), "{}").unwrap();
    let ids: Vec<String> = detect(&d).into_iter().map(|c| c.id).collect();
    assert!(ids.contains(&"bun/typecheck".to_string()));
    assert!(ids.contains(&"bun/lint".to_string()));
    assert!(ids.contains(&"bun/test".to_string()));

    let d2 = scratch();
    std::fs::write(d2.join("Cargo.toml"), "[package]").unwrap();
    let ids: Vec<String> = detect(&d2).into_iter().map(|c| c.id).collect();
    assert!(ids.contains(&"cargo/clippy".to_string()));

    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&d2);
}

#[test]
fn node_without_scripts_has_no_implicit_checks() {
    let d = scratch();
    std::fs::write(d.join("package.json"), "{}").unwrap();
    assert!(detect(&d).is_empty());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn javascript_checks_use_bun() {
    let d = scratch();
    std::fs::write(
        d.join("package.json"),
        r#"{"scripts":{"lint":"eslint .","test":"vitest"}}"#,
    )
    .unwrap();
    let checks = detect(&d);
    assert!(checks.iter().all(|check| check.program == "bun"));
    assert!(checks.iter().all(|check| {
        check.args.first().map(String::as_str) == Some("run")
            && check
                .args
                .get(1)
                .is_some_and(|script| check.id.ends_with(script))
    }));
    assert_eq!(
        checks.into_iter().map(|check| check.id).collect::<Vec<_>>(),
        ["bun/lint", "bun/test"]
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn lockfiles_pick_the_package_manager() {
    for (lockfile, manager) in [
        ("pnpm-lock.yaml", "pnpm"),
        ("yarn.lock", "yarn"),
        ("package-lock.json", "npm"),
    ] {
        let d = scratch();
        std::fs::write(d.join("package.json"), r#"{"scripts":{"test":"jest"}}"#).unwrap();
        std::fs::write(d.join(lockfile), "").unwrap();
        let check = detect(&d).into_iter().next().unwrap();
        assert_eq!(check.program, manager);
        assert_eq!(check.id, format!("{manager}/test"));
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[test]
fn detects_python_and_go() {
    let d = scratch();
    std::fs::write(d.join("pyproject.toml"), "[project]\nname='x'").unwrap();
    let ids: Vec<String> = detect(&d).into_iter().map(|c| c.id).collect();
    assert!(ids.contains(&"python/lint".to_string()));
    assert!(ids.contains(&"python/test".to_string()));

    let g = scratch();
    std::fs::write(g.join("go.mod"), "module x").unwrap();
    let ids: Vec<String> = detect(&g).into_iter().map(|c| c.id).collect();
    assert!(ids.contains(&"go/build".to_string()));
    assert!(ids.contains(&"go/vet".to_string()));
    assert!(ids.contains(&"go/test".to_string()));
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&g);
}

#[test]
fn malformed_package_has_no_implicit_checks() {
    let d = scratch();
    std::fs::write(d.join("package.json"), "not json").unwrap();
    assert!(detect(&d).is_empty());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn polyglot_check_ids_do_not_collide() {
    let d = scratch();
    std::fs::write(d.join("package.json"), r#"{"scripts":{"test":"bun test"}}"#).unwrap();
    std::fs::write(d.join("Cargo.toml"), "[package]").unwrap();
    let ids: Vec<String> = detect(&d).into_iter().map(|check| check.id).collect();
    let unique: std::collections::BTreeSet<&String> = ids.iter().collect();
    assert_eq!(ids.len(), unique.len());
    assert!(ids.contains(&"bun/test".to_string()));
    assert!(ids.contains(&"cargo/test".to_string()));
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
    assert_eq!(
        overall(&[mk(Status::Pass), mk(Status::Skipped)]),
        Status::Pass
    );
    assert_eq!(overall(&[mk(Status::Skipped)]), Status::Skipped);
    assert_eq!(overall(&[]), Status::Skipped);
}
