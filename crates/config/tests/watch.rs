use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn temp_file(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("asylumwatch{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir.join(name)
}

/// Wait (bounded) for the counter to reach at least `want`.
fn wait_for(count: &AtomicUsize, want: usize) -> bool {
    for _ in 0..200 {
        if count.load(Ordering::Relaxed) >= want {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn fires_on_change() {
    let path = temp_file("change.json");
    std::fs::write(&path, "{}").unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let handle = watch(path.clone(), Duration::from_millis(25), move || {
        seen.fetch_add(1, Ordering::Relaxed);
    });
    // An mtime with second granularity needs a distinct timestamp; rewriting
    // with new contents also bumps sub-second mtimes on every platform we run.
    std::thread::sleep(Duration::from_millis(60));
    std::fs::write(&path, "{ \"theme\": \"light\" }").unwrap();
    assert!(wait_for(&count, 1), "watcher never fired on a write");
    drop(handle);
}

#[test]
fn fires_on_appearing() {
    let path = temp_file("appear.json");
    let _ = std::fs::remove_file(&path);
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let handle = watch(path.clone(), Duration::from_millis(25), move || {
        seen.fetch_add(1, Ordering::Relaxed);
    });
    std::thread::sleep(Duration::from_millis(60));
    std::fs::write(&path, "{}").unwrap();
    assert!(wait_for(&count, 1), "watcher never fired on file creation");
    drop(handle);
}

#[test]
fn drop_stops_promptly() {
    let path = temp_file("stop.json");
    let handle = watch(path, Duration::from_secs(3600), || {});
    let start = std::time::Instant::now();
    drop(handle);
    assert!(start.elapsed() < Duration::from_secs(2));
}
