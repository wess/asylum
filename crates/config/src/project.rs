//! Per-project configuration: `asylum.toml` at a repository root.
//!
//! Where `settings.json` is the user's global config, `asylum.toml` is committed
//! with a repo and describes *that project* - the base branch its worktrees fork
//! from, which agents to fan out by default, setup commands to run when a
//! worktree is created, and environment overrides for agents. This is the
//! a committed, per-project config file. A malformed file yields defaults plus a
//! diagnostic (the loader never fails).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Diagnostic;

/// The project filename discovered at a repo root.
pub const PROJECT_FILE: &str = "asylum.toml";

/// Per-project configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    /// Branch new worktrees fork from; overrides the global default.
    pub base_branch: Option<String>,
    /// Agents fanned out by default for this project.
    pub default_agents: Vec<String>,
    /// Commands run once in a freshly-created worktree (install deps, etc.).
    pub setup: Vec<String>,
    /// Environment overrides applied to agents run in this project.
    pub env: BTreeMap<String, String>,
}

/// Load `asylum.toml` from `dir`. A missing file yields defaults with no
/// diagnostics; a broken one yields defaults plus a diagnostic.
pub fn load_project(dir: &Path) -> (ProjectConfig, Vec<Diagnostic>) {
    let path = dir.join(PROJECT_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_project(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            (ProjectConfig::default(), Vec::new())
        }
        Err(e) => (
            ProjectConfig::default(),
            vec![Diagnostic::new(
                "",
                format!("could not read {PROJECT_FILE}: {e}"),
            )],
        ),
    }
}

/// Parse project config from a TOML string.
pub fn parse_project(text: &str) -> (ProjectConfig, Vec<Diagnostic>) {
    if text.trim().is_empty() {
        return (ProjectConfig::default(), Vec::new());
    }
    match toml::from_str::<ProjectConfig>(text) {
        Ok(mut cfg) => {
            let diagnostics = crate::validate::validate_project(&mut cfg);
            (cfg, diagnostics)
        }
        Err(e) => (
            ProjectConfig::default(),
            vec![Diagnostic::new("", e.message().to_string())],
        ),
    }
}

#[cfg(test)]
#[path = "../tests/project.rs"]
mod tests;
