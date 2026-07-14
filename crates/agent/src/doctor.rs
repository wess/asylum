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
    }
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
