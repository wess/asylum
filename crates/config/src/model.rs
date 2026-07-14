//! The typed settings schema.
//!
//! Every field carries a serde default so a partial (or empty) `settings.json`
//! still deserializes into a complete [`Settings`]. `#[serde(default)]` on the
//! struct fills absent keys; per-field defaults set the compiled-in values.

use serde::{Deserialize, Serialize};

/// The full resolved configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Named color theme for the chrome (guise theme selection).
    pub theme: String,
    /// Where per-task worktrees are created, relative to a project root.
    pub worktree_dir: String,
    /// Ids of the agents (from the `agent` registry) enabled by default when a
    /// task is fanned out. Empty means "ask each time".
    pub default_agents: Vec<String>,
    /// How many agents may run concurrently across all tasks. 0 = unlimited.
    pub max_parallel_runs: u32,
    /// Stop an agent after this many minutes. 0 = no timeout.
    pub run_timeout_minutes: u32,
    /// Per-agent overrides keyed by agent id.
    pub agents: std::collections::BTreeMap<String, AgentPrefs>,
    /// Bring-your-own agents: definitions added on top of the built-in catalog.
    pub custom_agents: Vec<CustomAgent>,
    /// Built-in editor preferences.
    pub editor: EditorPrefs,
    /// Keybindings as `chord=action` strings, layered over the defaults.
    pub keybindings: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            worktree_dir: ".asylum/worktrees".to_string(),
            default_agents: Vec::new(),
            max_parallel_runs: 4,
            run_timeout_minutes: 60,
            agents: std::collections::BTreeMap::new(),
            custom_agents: Vec::new(),
            editor: EditorPrefs::default(),
            keybindings: Vec::new(),
        }
    }
}

/// Per-agent user overrides.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentPrefs {
    /// Override the launch program (e.g. a wrapper script).
    pub program: Option<String>,
    /// Extra arguments appended to the agent's command line.
    pub extra_args: Vec<String>,
    /// Force-enable or disable this agent regardless of `default_agents`.
    pub enabled: Option<bool>,
}

/// A user-defined agent added to the catalog ("bring your own agent").
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CustomAgent {
    /// Stable id (used in fan-out, branch names, and the store).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Single-glyph icon.
    pub icon: String,
    /// Program to launch (looked up on PATH).
    pub program: String,
    /// Argument template; `{prompt}` is substituted under `arg` delivery.
    pub args: Vec<String>,
    /// How the prompt is delivered: `"arg"` (default) or `"stdin"`.
    pub delivery: String,
}

/// Built-in code-editor preferences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EditorPrefs {
    pub font_family: String,
    pub font_size: f32,
    pub tab_width: u32,
    pub autosave: bool,
}

impl Default for EditorPrefs {
    fn default() -> Self {
        Self {
            font_family: "monospace".to_string(),
            font_size: 13.0,
            tab_width: 4,
            autosave: true,
        }
    }
}
