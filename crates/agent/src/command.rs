//! Resolve an agent + user prefs + prompt into a concrete spawn spec.

use config::AgentPrefs;

use crate::registry::{Agent, Delivery};

/// Everything the host needs to launch an agent on a pty: the program, the
/// fully-substituted argument list, the working directory, and (for
/// [`Delivery::Stdin`] agents) the bytes to feed to stdin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    /// Present when the prompt is delivered over stdin.
    pub stdin: Option<String>,
    /// Environment overrides applied on top of the inherited environment - the
    /// host uses this to inject the control-surface variables (`ASYLUM_RUN_ID`,
    /// `ASYLUM_CONTROL_URL`, …) so an agent can orchestrate the fleet.
    pub env: Vec<(String, String)>,
}

/// The `{prompt}` substitution token in an agent's argument template.
const TOKEN: &str = "{prompt}";

/// Build the spawn spec for `agent` running `prompt` in `cwd`, applying any user
/// `prefs` (program override + appended extra args).
pub fn build(agent: &Agent, prefs: Option<&AgentPrefs>, prompt: &str, cwd: &str) -> SpawnSpec {
    let program = prefs
        .and_then(|p| p.program.clone())
        .unwrap_or_else(|| agent.program.clone());

    let mut args: Vec<String> = Vec::new();
    let mut substituted = false;
    for tmpl in &agent.args {
        if tmpl == TOKEN && agent.delivery == Delivery::Arg {
            args.push(prompt.to_string());
            substituted = true;
        } else if tmpl.contains(TOKEN) && agent.delivery == Delivery::Arg {
            args.push(tmpl.replace(TOKEN, prompt));
            substituted = true;
        } else if tmpl == TOKEN {
            // Stdin delivery: drop the token entirely.
            continue;
        } else {
            args.push(tmpl.clone());
        }
    }

    // Arg-delivery agent whose template lacked a token: append the prompt.
    if agent.delivery == Delivery::Arg && !substituted {
        args.push(prompt.to_string());
    }

    if let Some(prefs) = prefs {
        args.extend(prefs.extra_args.iter().cloned());
    }

    let stdin = match agent.delivery {
        Delivery::Stdin => Some(prompt.to_string()),
        Delivery::Arg => None,
    };

    SpawnSpec {
        program,
        args,
        cwd: cwd.to_string(),
        stdin,
        env: Vec::new(),
    }
}

impl SpawnSpec {
    /// Attach environment overrides (chainable), replacing any already set.
    pub fn with_env(mut self, env: Vec<(String, String)>) -> Self {
        self.env = env;
        self
    }

    /// A shell-ish preview of the command, for display in the UI (not for
    /// execution - the host runs `program`/`args` directly, no shell).
    pub fn preview(&self) -> String {
        let mut parts = vec![self.program.clone()];
        parts.extend(self.args.iter().map(|a| {
            if a.chars().any(char::is_whitespace) {
                format!("\"{a}\"")
            } else {
                a.clone()
            }
        }));
        parts.join(" ")
    }
}

#[cfg(test)]
#[path = "../tests/command.rs"]
mod tests;
