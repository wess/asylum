//! Path resolution and settings loading.

use std::path::PathBuf;

use crate::jsonc;
use crate::model::Settings;
use crate::Diagnostic;

/// The outcome of a load: the resolved settings plus any non-fatal diagnostics.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub settings: Settings,
    pub diagnostics: Vec<Diagnostic>,
}

/// The default settings file path: `$XDG_CONFIG_HOME/asylum/settings.json`,
/// falling back to `~/.config/asylum/settings.json`.
pub fn default_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("asylum").join("settings.json")
}

/// Load settings from `path`. A missing file yields the defaults with no
/// diagnostics; a present-but-broken file yields the defaults plus diagnostics.
pub fn load(path: &std::path::Path) -> Loaded {
    match std::fs::read_to_string(path) {
        Ok(src) => load_str(&src),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Loaded {
            settings: Settings::default(),
            diagnostics: Vec::new(),
        },
        Err(e) => Loaded {
            settings: Settings::default(),
            diagnostics: vec![Diagnostic::new("", format!("could not read settings: {e}"))],
        },
    }
}

/// Parse settings from a JSON(-with-comments) string. Never fails: a parse
/// error returns defaults plus one diagnostic pointing at the failure. A field
/// with the wrong type surfaces as a diagnostic naming that key.
pub fn load_str(src: &str) -> Loaded {
    let cleaned = jsonc::strip(src);
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return Loaded {
            settings: Settings::default(),
            diagnostics: Vec::new(),
        };
    }
    match serde_json::from_str::<Settings>(&cleaned) {
        Ok(settings) => Loaded {
            settings,
            diagnostics: Vec::new(),
        },
        Err(e) => {
            // Try to name the offending key from serde's message when possible.
            let key = extract_key(&e.to_string());
            Loaded {
                settings: Settings::default(),
                diagnostics: vec![Diagnostic::new(key, e.to_string())],
            }
        }
    }
}

/// Pull a key name out of a serde_json error like `unknown field \`foo\`, ...`.
fn extract_key(msg: &str) -> String {
    for marker in ["unknown field `", "missing field `", "invalid type"] {
        if let Some(rest) = msg.split_once(marker).map(|(_, r)| r) {
            if marker == "invalid type" {
                return String::new();
            }
            if let Some(end) = rest.find('`') {
                return rest[..end].to_string();
            }
        }
    }
    String::new()
}

#[cfg(test)]
#[path = "../tests/load.rs"]
mod tests;
