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
///
/// This file lives *in the repository*, so everything here is controlled by
/// whoever wrote the repository rather than by the user who opened it. Two of
/// its fields cause execution, and both are withheld until the repository is
/// trusted - see [`Trust`] and [`ProjectConfig::with_trust`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    /// Branch new worktrees fork from; overrides the global default.
    pub base_branch: Option<String>,
    /// Agents fanned out by default for this project.
    pub default_agents: Vec<String>,
    /// Commands run once in a freshly-created worktree (install deps, etc.).
    ///
    /// **Executes.** These run through a login shell with the user's full
    /// privileges. Withheld unless [`Trust::Trusted`].
    pub setup: Vec<String>,
    /// Environment overrides applied to agents run in this project.
    ///
    /// **Executes.** Injected into agent and setup processes, where entries like
    /// `PATH`, `NODE_OPTIONS` or `GIT_SSH_COMMAND` turn into code execution.
    /// Withheld unless [`Trust::Trusted`].
    pub env: BTreeMap<String, String>,
}

/// Whether the user has trusted a repository to run its own commands.
///
/// Opening a repository is not consent to execute it. This is the deliberate
/// opt-in that releases the executable parts of [`ProjectConfig`], and it is a
/// distinct type rather than a `bool` so a call site cannot pass the wrong
/// argument without noticing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// Nothing the repository declares may execute.
    Untrusted,
    /// The user reviewed what this repository runs and accepted it.
    Trusted,
}

impl Trust {
    /// Trust derived from a stored stamp (unix seconds; 0 = never trusted).
    pub fn from_stamp(trusted_at: i64) -> Trust {
        if trusted_at > 0 {
            Trust::Trusted
        } else {
            Trust::Untrusted
        }
    }

    /// Whether execution is permitted.
    pub fn allows_execution(self) -> bool {
        self == Trust::Trusted
    }
}

impl ProjectConfig {
    /// The config as it may actually be used, given `trust`.
    ///
    /// Untrusted strips the executable fields and keeps the inert ones, so an
    /// untrusted repository still contributes its base branch and default agent
    /// list - neither of which runs anything - while its commands and
    /// environment are dropped rather than merely ignored downstream.
    pub fn with_trust(mut self, trust: Trust) -> Self {
        if !trust.allows_execution() {
            self.setup.clear();
            self.env.clear();
        }
        self
    }

    /// Whether this config declares anything that would execute. Drives the
    /// trust prompt, which must state what it is asking permission for.
    pub fn declares_execution(&self) -> bool {
        !self.setup.is_empty() || !self.env.is_empty()
    }
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
