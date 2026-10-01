//! The PATH local servers start with. An app opened from Finder gets only
//! the system PATH, so `npx` and `uvx` from Homebrew, asdf, or nvm aren't
//! found. Ask the user's login shell once and add the usual install
//! locations as a fallback.

use std::sync::OnceLock;

const COMMON: [&str; 5] = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin"];

pub fn login() -> &'static str {
  static PATH: OnceLock<String> = OnceLock::new();
  PATH.get_or_init(|| merge(&shell_path().unwrap_or_default(), &std::env::var("PATH").unwrap_or_default(), &home_bins()))
}

fn shell_path() -> Option<String> {
  let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
  let out = std::process::Command::new(shell).args(["-lic", "printf '%s' \"$PATH\""]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
  let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
  (out.status.success() && !p.is_empty()).then_some(p)
}

fn home_bins() -> Vec<String> {
  let home = std::env::var("HOME").unwrap_or_default();
  [".local/bin", ".cargo/bin", ".asdf/shims", ".volta/bin", ".bun/bin"].iter().map(|d| format!("{home}/{d}")).collect()
}

/// Login PATH first, then the current one, then common locations; no repeats.
pub fn merge(login: &str, current: &str, extra: &[String]) -> String {
  let mut seen = Vec::<String>::new();
  for dir in login.split(':').chain(current.split(':')).map(str::to_string).chain(extra.iter().cloned()).chain(COMMON.iter().map(|s| s.to_string())) {
    if !dir.is_empty() && !seen.contains(&dir) {
      seen.push(dir);
    }
  }
  seen.join(":")
}

#[cfg(test)]
#[path = "../tests/path.rs"]
mod tests;
