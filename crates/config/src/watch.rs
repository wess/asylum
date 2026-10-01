//! Live reload: watch the settings file's directory (editors replace files
//! by rename, which a file watch would lose) and call back on changes to it.

use anyhow::Result;
use notify::{RecursiveMode, Watcher};
use std::path::{Path, PathBuf};

pub struct WatchHandle {
  _watcher: notify::RecommendedWatcher,
}

pub fn watch(path: &Path, on_change: impl Fn() + Send + 'static) -> Result<WatchHandle> {
  let target: PathBuf = path.to_path_buf();
  let dir = target.parent().map(Path::to_path_buf).unwrap_or_default();
  std::fs::create_dir_all(&dir)?;
  let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
    if let Ok(ev) = res {
      if ev.paths.iter().any(|p| p.file_name() == target.file_name()) {
        on_change();
      }
    }
  })?;
  watcher.watch(&dir, RecursiveMode::NonRecursive)?;
  Ok(WatchHandle { _watcher: watcher })
}
