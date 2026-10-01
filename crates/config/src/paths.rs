use crate::APP;
use std::path::PathBuf;

fn home() -> PathBuf {
  std::env::var_os("HOME")
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("."))
}

/// `$XDG_CONFIG_HOME/asylum`, else `~/.config/asylum`.
pub fn config_dir() -> PathBuf {
  match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
    Some(xdg) => PathBuf::from(xdg).join(APP),
    None => home().join(".config").join(APP),
  }
}

/// `agents.json`, not `settings.json`: another tool already keeps a
/// `settings.json` in `~/.config/asylum`, and this app must never touch it.
/// Older builds called it `bots.json`; it's moved over on first use.
pub fn settings_path() -> PathBuf {
  let dir = config_dir();
  let path = dir.join("agents.json");
  let old = dir.join("bots.json");
  if !path.exists() && old.exists() {
    let _ = std::fs::rename(&old, &path);
  }
  path
}

/// Where the database and each Bot's computer live:
/// `~/Library/Application Support/asylum` on macOS, `$XDG_DATA_HOME/asylum`
/// (or `~/.local/share/asylum`) elsewhere.
pub fn data_dir() -> PathBuf {
  if cfg!(target_os = "macos") {
    return home().join("Library/Application Support").join(APP);
  }
  match std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
    Some(xdg) => PathBuf::from(xdg).join(APP),
    None => home().join(".local/share").join(APP),
  }
}
