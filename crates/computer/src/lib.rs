//! The computer every Bot shares. One per user, persistent across sessions:
//! `workspace/` is the durable shared filesystem (what Bots call
//! `/workspace`), `browser/` the Chromium profile whose cookies and sign-ins
//! every Bot shares, `screens/<bot>/` each Bot's screenshots, and
//! `recordings/` Teach-a-task demos. Each Bot drives its own browser page (its
//! "screen"), so Bots can do computer use in parallel.

pub mod browser;
pub mod clip;
pub mod fs;
pub mod proxy;
pub mod redact;
pub mod sandbox;
pub mod shell;
pub mod web;

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Computer {
  pub root: PathBuf,
}

impl Computer {
  pub fn new(base: &Path) -> Self {
    Self {
      root: base.join("computer"),
    }
  }

  pub fn workspace(&self) -> PathBuf {
    self.root.join("workspace")
  }

  pub fn profile(&self) -> PathBuf {
    self.root.join("browser")
  }

  pub fn screens(&self, bot: &str) -> PathBuf {
    self.root.join("screens").join(bot)
  }

  pub fn recordings(&self) -> PathBuf {
    self.root.join("recordings")
  }

  pub fn snapshots(&self) -> PathBuf {
    self.root.join("snapshots")
  }

  pub fn ensure(&self) -> std::io::Result<()> {
    for dir in [self.workspace(), self.profile(), self.recordings(), self.snapshots()] {
      std::fs::create_dir_all(dir)?;
    }
    Ok(())
  }
}

/// Bytes used under `path`, recursively.
pub fn disk_usage(path: &Path) -> u64 {
  let Ok(meta) = std::fs::symlink_metadata(path) else {
    return 0;
  };
  if !meta.is_dir() {
    return meta.len();
  }
  std::fs::read_dir(path)
    .map(|rd| rd.flatten().map(|e| disk_usage(&e.path())).sum())
    .unwrap_or(0)
}

/// Free bytes on the volume holding `path`.
pub fn disk_free(path: &Path) -> Option<u64> {
  let out = std::process::Command::new("df").arg("-k").arg(path).output().ok()?;
  let text = String::from_utf8_lossy(&out.stdout);
  let line = text.lines().nth(1)?;
  let avail: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
  Some(avail * 1024)
}

pub fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
  if !from.exists() {
    return Ok(());
  }
  std::fs::create_dir_all(to)?;
  for entry in std::fs::read_dir(from)? {
    let entry = entry?;
    let dest = to.join(entry.file_name());
    if entry.file_type()?.is_dir() {
      copy_dir(&entry.path(), &dest)?;
    } else {
      std::fs::copy(entry.path(), dest)?;
    }
  }
  Ok(())
}
