//! Single-instance guard.
//!
//! Two app processes against the same data dir would each run interrupted-run
//! recovery (the second marks the first's live runs failed) and both drain the
//! followup and control queues, double-launching queued work. To prevent that
//! the app takes an exclusive advisory lock on a file beside the database and
//! holds it for its whole lifetime; a second launch cannot acquire it and
//! refuses to start.
//!
//! The lock is `flock(2)` on Unix / `LockFileEx` on Windows (via the standard
//! library's advisory file locking), tied to the open file handle: the OS
//! releases it the instant the process ends, so a crash leaves nothing stale to
//! clear and there is no pid to parse.

use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};

/// Lock filename, placed beside the database in the data dir.
const LOCK_FILE: &str = "instance.lock";

/// A held single-instance lock. The advisory lock lives on the open handle;
/// keeping the guard alive holds the lock, and dropping it releases it.
pub struct Guard {
    file: std::fs::File,
}

impl Drop for Guard {
    fn drop(&mut self) {
        // The OS also releases the lock when the handle closes; unlocking first
        // makes the release explicit and immediate.
        let _ = self.file.unlock();
    }
}

/// The lock path for a database file: `instance.lock` in the database's own
/// directory.
pub fn path_for(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .map(|dir| dir.join(LOCK_FILE))
        .unwrap_or_else(|| PathBuf::from(LOCK_FILE))
}

/// Try to take the single-instance lock at `path`.
///
/// - `Ok(Some(guard))` — the lock is ours; hold `guard` for the app's lifetime.
/// - `Ok(None)` — another live instance holds it; the caller should refuse to
///   start.
/// - `Err(_)` — the lock file could not be opened (e.g. an unwritable data dir).
///
/// The lock file is created if absent; a pre-existing file left behind by a
/// crashed instance is reused, because the advisory lock — not the file's
/// existence — is the gate. The parent directory must already exist.
pub fn acquire(path: &Path) -> io::Result<Option<Guard>> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(Guard { file })),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}

#[cfg(test)]
#[path = "../tests/lock.rs"]
mod tests;
