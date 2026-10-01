use crate::jsonc;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
  #[default]
  System,
  Light,
  Dark,
}

/// Whether Bots may run commands on this machine (outside the computer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LocalExec {
  #[default]
  Ask,
  Always,
  Never,
}

/// How much a Bot may do before asking. `Ask` pauses on every consequential
/// action (sending, purchasing, deleting, posting); `Auto` only on the ones a
/// Bot's own rules mark as needing approval.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Approval {
  #[default]
  Ask,
  Auto,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Settings {
  pub appearance: Appearance,
  pub theme: String,
  pub model: String,
  pub fast_model: String,
  pub approval: Approval,
  pub notifications: bool,
  pub notification_sound: bool,
  pub keep_awake: bool,
  pub background_work: bool,
  pub max_parallel_bots: u32,
  pub max_steps: u32,
  pub font_size: f32,
  pub terminal_font: String,
  pub send_on_enter: bool,
  pub show_reasoning: bool,
  pub memory: bool,
  pub launch_at_login: bool,
  pub browser_home: String,
  pub user_name: String,
  pub language: String,
  pub timezone: String,
  pub auto_review: bool,
  pub local_exec: LocalExec,
  pub security_keys: bool,
  pub weekly_limit: u64,
  pub on_demand: bool,
  pub monthly_limit: u64,
  pub webhook_port: u16,
  pub automatic_updates: bool,
  pub microphone: String,
  pub voice: String,
  pub voice_speed: f32,
  pub voice_language: String,
  pub zoom: f32,
  pub compact_sidebar: bool,
  pub isolate_computer: bool,
  /// Named provider profiles; `provider` is the default, `model` its model.
  pub providers: Vec<crate::provider::Profile>,
  pub provider: String,
  /// The model for quick background work (Auto-review, memory, names).
  pub fast_provider: String,
  pub retries: u32,
  /// A folder (e.g. in iCloud Drive) sidebar sections sync through.
  pub sync_folder: String,
  /// Hibernate the computer after this many idle minutes (0: never).
  pub hibernate_minutes: u32,
  /// Dictation, voice chat, and voice memos. Off (and hidden) by default.
  pub voice_enabled: bool,
  /// OTLP/HTTP collector base URL for telemetry (empty: off).
  pub otel_endpoint: String,
  /// Local Admin API port (0: off). Its token lives in the keychain.
  pub admin_port: u16,
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      appearance: Appearance::System,
      theme: "default".into(),
      model: "grok-4".into(),
      fast_model: "grok-3-mini".into(),
      approval: Approval::Ask,
      notifications: true,
      notification_sound: true,
      keep_awake: false,
      background_work: true,
      max_parallel_bots: 4,
      max_steps: 40,
      font_size: 14.0,
      terminal_font: "Menlo".into(),
      send_on_enter: true,
      show_reasoning: false,
      memory: true,
      launch_at_login: false,
      browser_home: "https://duckduckgo.com".into(),
      user_name: String::new(),
      language: "system".into(),
      timezone: "auto".into(),
      auto_review: true,
      local_exec: LocalExec::Ask,
      security_keys: true,
      weekly_limit: 5_000_000,
      on_demand: false,
      monthly_limit: 20_000_000,
      webhook_port: 47821,
      automatic_updates: true,
      microphone: String::new(),
      voice: "Ara".into(),
      voice_speed: 1.0,
      voice_language: "auto".into(),
      zoom: 1.0,
      compact_sidebar: false,
      isolate_computer: true,
      providers: Vec::new(),
      provider: String::new(),
      fast_provider: String::new(),
      retries: 3,
      sync_folder: String::new(),
      hibernate_minutes: 30,
      voice_enabled: false,
      otel_endpoint: String::new(),
      admin_port: 0,
    }
  }
}

/// A load result: the settings (never absent — bad values fall back to their
/// defaults) plus human-readable problems to surface in the UI.
#[derive(Clone, Debug, Default)]
pub struct Load {
  pub settings: Settings,
  pub diagnostics: Vec<String>,
}

pub fn parse(text: &str) -> Load {
  let clean = jsonc::strip(text);
  if clean.trim().is_empty() {
    return Load::default();
  }
  let root = match serde_json::from_str::<Value>(&clean) {
    Ok(Value::Object(m)) => m,
    Ok(_) => return fail("bots.json must be an object"),
    Err(e) => return fail(&format!("bots.json: {e}")),
  };
  let defaults = match serde_json::to_value(Settings::default()) {
    Ok(Value::Object(m)) => m,
    _ => Map::new(),
  };
  let mut merged = defaults.clone();
  let mut diagnostics = Vec::new();
  for (key, value) in root {
    let Some(default) = defaults.get(&key) else {
      diagnostics.push(format!("unknown setting \"{key}\""));
      continue;
    };
    let mut probe = defaults.clone();
    probe.insert(key.clone(), value.clone());
    if serde_json::from_value::<Settings>(Value::Object(probe)).is_ok() {
      merged.insert(key, value);
    } else {
      diagnostics.push(format!("\"{key}\": invalid value {value}, using {default}"));
    }
  }
  let settings = serde_json::from_value(Value::Object(merged)).unwrap_or_default();
  Load {
    settings,
    diagnostics,
  }
}

fn fail(msg: &str) -> Load {
  Load {
    settings: Settings::default(),
    diagnostics: vec![msg.to_string()],
  }
}

pub fn load(path: &Path) -> Load {
  match std::fs::read_to_string(path) {
    Ok(text) => parse(&text),
    Err(_) => Load::default(),
  }
}

/// Write only the keys that differ from the defaults, so the file stays a
/// short list of the user's actual choices.
pub fn save(path: &Path, settings: &Settings) -> Result<()> {
  let defaults = serde_json::to_value(Settings::default())?;
  let current = serde_json::to_value(settings)?;
  let mut out = Map::new();
  if let (Value::Object(cur), Value::Object(def)) = (current, defaults) {
    for (k, v) in cur {
      if def.get(&k) != Some(&v) {
        out.insert(k, v);
      }
    }
  }
  if let Some(dir) = path.parent() {
    std::fs::create_dir_all(dir)?;
  }
  let tmp = path.with_extension("json.tmp");
  std::fs::write(&tmp, serde_json::to_string_pretty(&Value::Object(out))? + "\n")?;
  std::fs::rename(tmp, path)?;
  Ok(())
}

#[cfg(test)]
#[path = "../tests/settings.rs"]
mod tests;
