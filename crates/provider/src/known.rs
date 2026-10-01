//! Models a CLI can use. There is no discovery protocol for process
//! providers, so the list is the aliases the CLI documents plus the model
//! the tool is already configured with on this machine.

use std::path::PathBuf;

fn home() -> PathBuf {
  std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn claude_configured() -> Option<String> {
  let text = std::fs::read_to_string(home().join(".claude/settings.json")).ok()?;
  let v: serde_json::Value = serde_json::from_str(&text).ok()?;
  v["model"].as_str().map(str::to_string)
}

fn codex_configured() -> Option<String> {
  let text = std::fs::read_to_string(home().join(".codex/config.toml")).ok()?;
  let v: toml::Value = toml::from_str(&text).ok()?;
  v.get("model")?.as_str().map(str::to_string)
}

pub fn for_preset(preset: &str) -> Vec<String> {
  let (mut base, configured): (Vec<&str>, Option<String>) = match preset {
    "claude-code" => (vec!["fable", "opus", "sonnet", "haiku"], claude_configured()),
    "codex" => (vec!["gpt-5-codex", "gpt-5"], codex_configured()),
    _ => (Vec::new(), None),
  };
  let mut out: Vec<String> = Vec::new();
  if let Some(c) = configured {
    out.push(c);
  }
  base.retain(|b| !out.iter().any(|o| o == b));
  out.extend(base.into_iter().map(str::to_string));
  out
}
