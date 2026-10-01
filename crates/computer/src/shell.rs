//! Run a command with a login shell, a timeout, captured output, secret
//! values injected as environment variables, and those values redacted from
//! what comes back.

use crate::clip::clip;
use crate::redact::redact;
use crate::sandbox;
use anyhow::Result;
use serde::Serialize;
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Output {
  pub code: Option<i32>,
  pub stdout: String,
  pub stderr: String,
  pub timed_out: bool,
}

impl Output {
  pub fn render(&self) -> String {
    let mut s = String::new();
    if !self.stdout.is_empty() {
      s.push_str(&self.stdout);
    }
    if !self.stderr.is_empty() {
      if !s.is_empty() {
        s.push('\n');
      }
      s.push_str("[stderr]\n");
      s.push_str(&self.stderr);
    }
    if self.timed_out {
      s.push_str("\n[timed out]");
    }
    match self.code {
      Some(0) if s.is_empty() => "(no output)".into(),
      Some(0) => s,
      Some(c) => format!("{s}\n[exit {c}]"),
      None => s,
    }
  }
}

/// Where and how a command runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
  /// The shared computer: cwd is the workspace, writes confined to it.
  Computer,
  /// The user's own machine, unconfined, cwd their home.
  Local,
}

pub struct Spec<'a> {
  pub command: &'a str,
  pub cwd: &'a Path,
  pub place: Place,
  pub env: &'a [(String, String)],
  pub timeout: Duration,
  pub net: sandbox::Net,
}

pub fn shell() -> String {
  std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into())
}

pub async fn run(spec: Spec<'_>) -> Result<Output> {
  std::fs::create_dir_all(spec.cwd)?;
  let mut cmd = if spec.place == Place::Computer && sandbox::available() {
    let mut c = Command::new("/usr/bin/sandbox-exec");
    c.arg("-p").arg(sandbox::profile_with(spec.cwd, spec.net)).arg(shell());
    c
  } else if spec.net != sandbox::Net::Open && sandbox::available() {
    let mut c = Command::new("/usr/bin/sandbox-exec");
    c.arg("-p").arg(sandbox::net_only(spec.net)).arg(shell());
    c
  } else {
    Command::new(shell())
  };
  cmd
    .arg("-lc")
    .arg(spec.command)
    .current_dir(spec.cwd)
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .kill_on_drop(true);
  if spec.place == Place::Computer {
    cmd.env("WORKSPACE", spec.cwd);
  }
  if let sandbox::Net::Proxy(port) = spec.net {
    let url = format!("http://127.0.0.1:{port}");
    for k in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"] {
      cmd.env(k, &url);
    }
    cmd.env("NO_PROXY", "localhost,127.0.0.1").env("no_proxy", "localhost,127.0.0.1");
  }
  for (k, v) in spec.env {
    cmd.env(k, v);
  }
  let secrets: Vec<String> = spec.env.iter().map(|(_, v)| v.clone()).collect();
  let child = cmd.spawn()?;
  match tokio::time::timeout(spec.timeout, child.wait_with_output()).await {
    Ok(out) => {
      let out = out?;
      Ok(Output {
        code: out.status.code(),
        stdout: clip(&redact(&String::from_utf8_lossy(&out.stdout), &secrets), 24_000),
        stderr: clip(&redact(&String::from_utf8_lossy(&out.stderr), &secrets), 8_000),
        timed_out: false,
      })
    }
    Err(_) => Ok(Output {
      code: None,
      stdout: String::new(),
      stderr: String::new(),
      timed_out: true,
    }),
  }
}

#[cfg(test)]
#[path = "../tests/shell.rs"]
mod tests;
