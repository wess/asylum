//! Scrollback persistence.
//!
//! Terminal scrollback is restored when the app restarts. A run's scrollback
//! is captured as text and written to a per-run file; on the next launch it is
//! read back and can be replayed above a fresh terminal. This module is the
//! pure persistence half - the [`Runner`](crate::Runner) supplies the text via
//! [`Runner::history_text`](crate::Runner::history_text).

use std::io;
use std::path::Path;

/// Save `text` as the scrollback at `path`, creating parent directories.
pub fn save(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)
}

/// Load persisted scrollback from `path`, or `None` if there is none.
pub fn load(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Remove a persisted scrollback file (e.g. when a run is discarded).
pub fn clear(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
