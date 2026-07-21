use super::*;

use std::fs;

/// A unique lock path in the OS temp dir (its parent already exists). The tag
/// keeps parallel tests from sharing a path.
fn temp_path(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("asylumlock{tag}{}{nanos}", std::process::id()))
}

#[test]
fn acquire_succeeds_on_a_fresh_path() {
    let path = temp_path("fresh");
    let guard = acquire(&path).expect("open lock file");
    assert!(guard.is_some(), "a fresh lock should be acquired");
    drop(guard);
    let _ = fs::remove_file(&path);
}

#[test]
fn second_acquire_is_refused_while_held() {
    let path = temp_path("held");
    let first = acquire(&path).expect("open lock file");
    assert!(first.is_some(), "first acquire should win");
    // A second, independent open of the same path conflicts: flock is
    // per-open-file-description, so this holds even within one process.
    let second = acquire(&path).expect("open lock file");
    assert!(second.is_none(), "a second instance must be refused");
    drop(first);
    let _ = fs::remove_file(&path);
}

#[test]
fn dropping_the_guard_releases_the_lock() {
    let path = temp_path("release");
    let first = acquire(&path).expect("open lock file").expect("acquire");
    drop(first);
    // With the guard gone the lock is free again.
    let again = acquire(&path).expect("open lock file");
    assert!(again.is_some(), "re-acquire after release should win");
    drop(again);
    let _ = fs::remove_file(&path);
}

#[test]
fn a_preexisting_lock_file_does_not_block() {
    let path = temp_path("stale");
    // A lock file left behind by a crashed instance: the file exists but
    // nothing holds the advisory lock.
    fs::write(&path, b"").expect("create leftover lock file");
    let guard = acquire(&path).expect("open lock file");
    assert!(
        guard.is_some(),
        "a leftover lock file must not block acquisition"
    );
    drop(guard);
    let _ = fs::remove_file(&path);
}

#[test]
fn path_for_places_the_lock_beside_the_database() {
    let db = std::path::Path::new("/data/asylum/workspace.sqlite");
    assert_eq!(
        path_for(db),
        std::path::Path::new("/data/asylum/instance.lock")
    );
}
