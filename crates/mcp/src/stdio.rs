//! A server run as a child process, speaking newline-delimited JSON-RPC.

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio as Pipe;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

pub struct Stdio {
  _child: Child,
  stdin: Mutex<ChildStdin>,
  stdout: Mutex<tokio::io::Lines<BufReader<ChildStdout>>>,
}

impl Stdio {
  pub fn spawn(command: &str, args: &[String], env: &HashMap<String, String>, cwd: Option<&std::path::Path>) -> Result<Self> {
    let mut cmd = Command::new(command);
    // Server env wins, but a PATH it doesn't set comes from the login shell.
    cmd
      .env("PATH", crate::path::login())
      .args(args)
      .envs(env)
      .stdin(Pipe::piped())
      .stdout(Pipe::piped())
      .stderr(Pipe::null())
      .kill_on_drop(true);
    if let Some(dir) = cwd {
      cmd.current_dir(dir);
    }
    let mut child = cmd.spawn().with_context(|| format!("could not start {command}"))?;
    let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
    Ok(Self {
      _child: child,
      stdin: Mutex::new(stdin),
      stdout: Mutex::new(BufReader::new(stdout).lines()),
    })
  }

  pub async fn send(&self, msg: &Value) -> Result<()> {
    let mut line = serde_json::to_string(msg)?;
    line.push('\n');
    let mut stdin = self.stdin.lock().await;
    stdin.write_all(line.as_bytes()).await?;
    stdin.flush().await?;
    Ok(())
  }

  /// Send a request and read until the response with the same id arrives,
  /// skipping notifications and server requests in between.
  pub async fn call(&self, msg: &Value) -> Result<Value> {
    let id = msg["id"].clone();
    self.send(msg).await?;
    let mut out = self.stdout.lock().await;
    loop {
      let line = out.next_line().await?.ok_or_else(|| anyhow!("the server exited"))?;
      let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
      if v.get("id") == Some(&id) && (v.get("result").is_some() || v.get("error").is_some()) {
        return Ok(v);
      }
    }
  }
}
