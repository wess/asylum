//! Agent executable readiness checks used by onboarding and dispatch.

use std::path::{Path, PathBuf};

use crate::registry::Agent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub agent: String,
    pub program: String,
    pub path: Option<PathBuf>,
    /// Set by the host when this agent has completed a real run successfully.
    pub verified: bool,
    /// A copy-pasteable install hint for this agent's CLI, when known. Shown to
    /// a user who has the agent enabled but not installed.
    pub install: Option<&'static str>,
}

impl Report {
    pub fn ready(&self) -> bool {
        self.path.is_some()
    }

    pub fn verified(&self) -> bool {
        self.ready() && self.verified
    }
}

pub fn inspect(agent: &Agent) -> Report {
    Report {
        agent: agent.id.clone(),
        program: agent.program.clone(),
        path: find_program(&agent.program),
        verified: false,
        install: install_hint(&agent.id),
    }
}

/// A best-effort install command or docs URL for a known agent CLI. Returns
/// `None` for agents we don't have a canonical install line for.
pub fn install_hint(id: &str) -> Option<&'static str> {
    Some(match id {
        "claude-code" => {
            "npm i -g @anthropic-ai/claude-code  ·  https://docs.claude.com/claude-code"
        }
        "codex" => "npm i -g @openai/codex  ·  https://github.com/openai/codex",
        "opencode" => "curl -fsSL https://opencode.ai/install | bash",
        "gemini" => "npm i -g @google/gemini-cli",
        "grok" => "npm i -g @vibe-kit/grok-cli",
        "cursor-agent" => "curl https://cursor.com/install -fsS | bash",
        "copilot" => "npm i -g @github/copilot",
        "aider" => "python -m pip install aider-install && aider-install  ·  https://aider.chat",
        "continue" => "npm i -g @continuedev/cli",
        "cline" => "npm i -g cline",
        "goose" => {
            "curl -fsSL https://raw.githubusercontent.com/block/goose/main/download_cli.sh | bash"
        }
        "amp" => "npm i -g @sourcegraph/amp",
        "qwen-code" => "npm i -g @qwen-code/qwen-code",
        "codebuff" => "npm i -g codebuff",
        _ => return None,
    })
}

pub fn find_program(program: &str) -> Option<PathBuf> {
    if program.trim().is_empty() {
        return None;
    }
    let direct = Path::new(program);
    if direct.components().count() > 1 {
        return executable(direct).then(|| direct.to_path_buf());
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|dir| dir.join(program))
        .find(|path| executable(path))
}

fn executable(path: &Path) -> bool {
    let Ok(meta) = path.metadata() else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
#[path = "../tests/doctor.rs"]
mod tests;
