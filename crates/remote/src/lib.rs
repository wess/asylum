//! SSH remote worktrees and port forwarding.
//!
//! A worktree can run on a remote host over SSH, with auto-reconnect and
//! port forwarding, caching the passphrase across connections. This crate builds
//! the `ssh` command lines for those flows — it does not run them, so the argv
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
            opt(&mut argv, format!("ServerAliveInterval={}", self.keepalive_secs));
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
    pub fn port_forward(&self, local_port: u16, remote_host: &str, remote_port: u16) -> Vec<String> {
        let mut argv = self.base();
        argv.push("-N".into());
        argv.push("-L".into());
        argv.push(format!("{local_port}:{remote_host}:{remote_port}"));
        argv.push(self.target());
        argv
    }

    /// Build the command to create a worktree on the remote host: it changes to
    /// `repo`, adds a worktree at `path` (optionally on a new `branch`).
    pub fn worktree_create(&self, repo: &str, path: &str, branch: Option<&str>) -> Vec<String> {
        let add = match branch {
            Some(b) if !b.is_empty() => format!("git worktree add -b {b} {path} HEAD"),
            _ => format!("git worktree add {path}"),
        };
        self.exec(&format!("cd {repo} && {add}"))
    }

    /// Build the command to remove a remote worktree.
    pub fn worktree_remove(&self, repo: &str, path: &str) -> Vec<String> {
        self.exec(&format!("cd {repo} && git worktree remove --force {path}"))
    }
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
