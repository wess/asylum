//! SSH remote worktrees and port forwarding.
//!
//! A worktree can run on a remote host over SSH, with auto-reconnect and
//! port forwarding, caching the passphrase across connections. This crate builds
//! the `ssh` command lines for those flows - it does not run them, so the argv
//! is fully unit-testable. Connection multiplexing (`ControlMaster`) provides
//! the passphrase caching; `ServerAliveInterval` (or `autossh`) provides
//! keepalive/auto-reconnect.

/// A remote host and its SSH connection parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    pub host: String,
    pub user: Option<String>,
    pub port: Option<u16>,
    /// Path to an identity (private key) file.
    pub identity: Option<String>,
    /// ControlMaster socket path for connection reuse (passphrase caching). When
    /// set, connections multiplex over one authenticated channel.
    pub control_path: Option<String>,
    /// Keepalive interval in seconds (ServerAliveInterval). 0 disables it.
    pub keepalive_secs: u32,
    /// Use `autossh` instead of `ssh` for automatic reconnection.
    pub autossh: bool,
}

impl Host {
    /// A minimal host with sensible reconnection defaults.
    pub fn new(host: impl Into<String>) -> Self {
        Host {
            host: host.into(),
            user: None,
            port: None,
            identity: None,
            control_path: None,
            keepalive_secs: 15,
            autossh: false,
        }
    }

    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }
    pub fn identity(mut self, path: impl Into<String>) -> Self {
        self.identity = Some(path.into());
        self
    }
    pub fn control_path(mut self, path: impl Into<String>) -> Self {
        self.control_path = Some(path.into());
        self
    }
    pub fn autossh(mut self, on: bool) -> Self {
        self.autossh = on;
        self
    }

    /// `user@host` or `host`.
    pub fn target(&self) -> String {
        match &self.user {
            Some(u) => format!("{u}@{}", self.host),
            None => self.host.clone(),
        }
    }

    /// The base argv up to (but not including) the target: program + `-o` opts.
    fn base(&self) -> Vec<String> {
        let mut argv = vec![if self.autossh { "autossh" } else { "ssh" }.to_string()];
        let opt = |argv: &mut Vec<String>, kv: String| {
            argv.push("-o".into());
            argv.push(kv);
        };
        if self.keepalive_secs > 0 {
            opt(
                &mut argv,
                format!("ServerAliveInterval={}", self.keepalive_secs),
            );
            opt(&mut argv, "ServerAliveCountMax=3".into());
        }
        if let Some(cp) = &self.control_path {
            opt(&mut argv, "ControlMaster=auto".into());
            opt(&mut argv, format!("ControlPath={cp}"));
            opt(&mut argv, "ControlPersist=600".into());
        }
        if let Some(port) = self.port {
            argv.push("-p".into());
            argv.push(port.to_string());
        }
        if let Some(id) = &self.identity {
            argv.push("-i".into());
            argv.push(id.clone());
        }
        argv
    }

    /// Build `ssh [opts] target -- <remote_cmd>`.
    pub fn exec(&self, remote_cmd: &str) -> Vec<String> {
        let mut argv = self.base();
        argv.push(self.target());
        argv.push(remote_cmd.to_string());
        argv
    }

    /// Build a local port-forward: `ssh -N -L local:remote_host:remote_port target`.
    pub fn port_forward(
        &self,
        local_port: u16,
        remote_host: &str,
        remote_port: u16,
    ) -> Vec<String> {
        let mut argv = self.base();
        argv.push("-N".into());
        argv.push("-L".into());
        argv.push(format!("{local_port}:{remote_host}:{remote_port}"));
        argv.push(self.target());
        argv
    }

    /// Build the command to create a worktree on the remote host: it changes to
    /// `repo`, adds a worktree at `path` (optionally on a new `branch`).
    ///
    /// The remote command is executed by the remote user's shell, so every
    /// interpolated value is POSIX-quoted ([`shell_quote`]) and cannot inject
    /// shell syntax. `repo`/`path` are additionally refused if empty or starting
    /// with `-` (which git would read as an option), and `branch` is validated
    /// against git ref rules ([`valid_branch`]). Returns an error rather than a
    /// dangerous command.
    pub fn worktree_create(
        &self,
        repo: &str,
        path: &str,
        branch: Option<&str>,
    ) -> Result<Vec<String>, String> {
        safe_pathish("repo", repo)?;
        safe_pathish("worktree path", path)?;
        let add = match branch {
            Some(b) if !b.is_empty() => {
                if !valid_branch(b) {
                    return Err(format!("invalid branch name: {b:?}"));
                }
                format!(
                    "git worktree add -b {} {} HEAD",
                    shell_quote(b),
                    shell_quote(path)
                )
            }
            _ => format!("git worktree add {}", shell_quote(path)),
        };
        Ok(self.exec(&format!("cd {} && {add}", shell_quote(repo))))
    }

    /// Build the command to remove a remote worktree. `repo`/`path` are quoted
    /// and refused if empty or option-like, as in [`Self::worktree_create`].
    pub fn worktree_remove(&self, repo: &str, path: &str) -> Result<Vec<String>, String> {
        safe_pathish("repo", repo)?;
        safe_pathish("worktree path", path)?;
        Ok(self.exec(&format!(
            "cd {} && git worktree remove --force {}",
            shell_quote(repo),
            shell_quote(path)
        )))
    }
}

/// POSIX-quote `s` so it is a single literal shell word: wrap in single quotes
/// and rewrite each embedded `'` as `'\''`. Inside single quotes the shell
/// treats every other byte literally, so spaces, quotes, `;`, `|`, `&`,
/// newlines, `$(...)`/backtick substitutions, and Unicode cannot change the
/// command's structure.
pub fn shell_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

/// A `repo`/`path` argument safe to embed in a remote git command: non-empty and
/// not starting with `-`. Quoting neutralizes shell metacharacters; this guards
/// the separate option-injection layer (a value like `--upload-pack=…` that git
/// would read as a flag). Note that quoting deliberately disables `~`/`$VAR`
/// expansion, so remote paths must be absolute.
fn safe_pathish(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.starts_with('-') {
        return Err(format!("{label} must not start with '-': {value:?}"));
    }
    Ok(())
}

/// Whether `name` is a safe git branch name to embed. Applies git's ref rules
/// plus a leading-`-` guard, so a branch can never be read as an option or carry
/// shell/control characters.
pub fn valid_branch(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.starts_with('/')
        && !name.starts_with('.')
        && !name.ends_with('/')
        && !name.ends_with(".lock")
        && !name.contains("..")
        && !name.contains("//")
        && !name.contains("@{")
        && name.chars().all(|c| {
            !c.is_control()
                && !c.is_whitespace()
                && !matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
        })
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
