//! Keybindings: a chord → action map layered over compiled defaults.
//!
//! Bindings are `chord=action` strings (e.g. `cmd-k=command_palette`), so user
//! config can add or override any of them. The
//! defaults name the ADE's core actions; [`Keymap::from_settings`] layers the
//! user's `keybindings` on top (later entries win, and `chord=` with an empty
//! action unbinds).

use std::collections::BTreeMap;

/// The ADE's default keybindings as `(chord, action)` pairs.
pub const DEFAULTS: &[(&str, &str)] = &[
    ("cmd-k", "command_palette"),
    ("cmd-p", "quick_open"),
    ("cmd-shift-f", "search"),
    ("cmd-n", "new_task"),
    ("cmd-enter", "run_fanout"),
    ("cmd-shift-r", "review_diff"),
    ("cmd-shift-m", "merge_winner"),
    ("cmd-b", "toggle_sidebar"),
    ("cmd-t", "new_terminal"),
    ("cmd-d", "split_right"),
    ("cmd-shift-d", "split_down"),
    ("cmd-j", "toggle_panel"),
    ("cmd-comma", "settings"),
    ("cmd-shift-a", "switch_account"),
    ("cmd-shift-n", "notifications"),
];

/// A resolved keymap.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Keymap {
    binds: BTreeMap<String, String>,
}

impl Keymap {
    /// The compiled defaults.
    pub fn defaults() -> Self {
        let mut binds = BTreeMap::new();
        for (chord, action) in DEFAULTS {
            binds.insert((*chord).to_string(), (*action).to_string());
        }
        Keymap { binds }
    }

    /// Defaults with the user's `keybindings` layered on top. A binding whose
    /// action is empty (`"cmd-k="`) removes that chord.
    pub fn from_settings(user: &[String]) -> Self {
        let mut map = Self::defaults();
        for entry in user {
            if let Some((chord, action)) = parse_binding(entry) {
                if action.is_empty() {
                    map.binds.remove(&chord);
                } else {
                    map.binds.insert(chord, action);
                }
            }
        }
        map
    }

    /// The action bound to `chord`, if any.
    pub fn action(&self, chord: &str) -> Option<&str> {
        self.binds.get(chord).map(String::as_str)
    }

    /// All bindings as (chord, action) pairs, sorted by chord.
    pub fn bindings(&self) -> impl Iterator<Item = (&str, &str)> {
        self.binds.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Number of active bindings.
    pub fn len(&self) -> usize {
        self.binds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.binds.is_empty()
    }
}

/// Parse a `chord=action` binding. The chord is normalized to lowercase; a
/// trailing `=` with no action is kept as an empty action (an unbind). Returns
/// `None` when there is no `=`.
pub fn parse_binding(entry: &str) -> Option<(String, String)> {
    let (chord, action) = entry.split_once('=')?;
    let chord = chord.trim().to_ascii_lowercase();
    if chord.is_empty() {
        return None;
    }
    Some((chord, action.trim().to_string()))
}

#[cfg(test)]
#[path = "../tests/keys.rs"]
mod tests;
