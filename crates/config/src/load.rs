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
///
/// Durable secrets can be kept out of `settings.json` entirely: any secret field
/// left empty is filled from an environment variable (`ASYLUM_LINEAR_TOKEN`,
/// `ASYLUM_COMPANION_TOKEN`), so the token can come from the user's shell or a
/// credential manager rather than a plaintext config file.
pub fn load(path: &std::path::Path) -> Loaded {
    let mut loaded = match std::fs::read_to_string(path) {
        Ok(src) => load_str(&src),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Loaded {
            settings: Settings::default(),
            diagnostics: Vec::new(),
        },
        Err(e) => Loaded {
            settings: Settings::default(),
            diagnostics: vec![Diagnostic::new("", format!("could not read settings: {e}"))],
        },
    };
    resolve_secrets(
        &mut loaded.settings,
        std::env::var("ASYLUM_LINEAR_TOKEN").ok(),
        std::env::var("ASYLUM_COMPANION_TOKEN").ok(),
    );
    loaded
}

/// Fill any empty secret field from its environment override. A configured
/// (non-empty) value always wins; a blank override is ignored. Pure over its
/// inputs so it is testable without touching the process environment.
pub(crate) fn resolve_secrets(
    settings: &mut Settings,
    linear_token: Option<String>,
    companion_token: Option<String>,
) {
    if settings.linear_token.trim().is_empty() {
        if let Some(v) = linear_token.filter(|v| !v.trim().is_empty()) {
            settings.linear_token = v;
        }
    }
    if settings.companion.token.trim().is_empty() {
        if let Some(v) = companion_token.filter(|v| !v.trim().is_empty()) {
            settings.companion.token = v;
        }
    }
}

/// Parse settings from a JSON(-with-comments) string. Never fails: a parse
/// error returns defaults plus one diagnostic pointing at the failure. A field
/// with the wrong type surfaces as a diagnostic naming that key.
///
/// A bad key costs only that key. `deny_unknown_fields` means one typo makes
/// serde reject the whole document, so falling back to `Settings::default()`
/// wholesale would silently throw away every *good* setting alongside it -
/// theme, keybindings, agent paths - which is a rough thing to happen while
/// someone is editing their settings in the app's own editor.
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
        Err(e) => salvage(&cleaned, e),
    }
}

/// Keep every top-level key that stands on its own, and turn the rest into
/// diagnostics.
///
/// Each key is checked in isolation against `Settings`, which works because the
/// struct is `#[serde(default)]`: a document naming one key deserializes if and
/// only if that key is good.
fn salvage(cleaned: &str, err: serde_json::Error) -> Loaded {
    // The document has to be a JSON object before any of it can be salvaged.
    // Anything else (a syntax error, a bare array) is unrecoverable as settings.
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(cleaned)
    else {
        return Loaded {
            settings: Settings::default(),
            diagnostics: vec![Diagnostic::new(
                extract_key(&err.to_string()),
                err.to_string(),
            )],
        };
    };

    let mut good = serde_json::Map::new();
    let mut diagnostics = Vec::new();
    for (key, value) in map {
        let mut probe = serde_json::Map::new();
        probe.insert(key.clone(), value.clone());
        match serde_json::from_value::<Settings>(serde_json::Value::Object(probe)) {
            Ok(_) => {
                good.insert(key, value);
            }
            Err(e) => diagnostics.push(Diagnostic::new(key, e.to_string())),
        }
    }

    match serde_json::from_value::<Settings>(serde_json::Value::Object(good)) {
        Ok(settings) => Loaded {
            settings,
            diagnostics,
        },
        // Every key passed alone but they fail together, so there is no subset
        // to trust. Report the original error against clean defaults.
        Err(e) => {
            diagnostics.insert(
                0,
                Diagnostic::new(extract_key(&e.to_string()), e.to_string()),
            );
            Loaded {
                settings: Settings::default(),
                diagnostics,
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
