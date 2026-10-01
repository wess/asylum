//! A coding-agent CLI as a provider: the transcript goes in on stdin, the
//! answer comes back on stdout — whole, as a JSON `result`, or streamed as
//! JSON lines. The command runs directly (never through a shell) in the
//! computer's workspace with `{model}` and `{workspace}` substituted.

use crate::streamjson::Lines;
use anyhow::{anyhow, bail, Context, Result};
use config::Output;
use futures::stream::{BoxStream, StreamExt};
use grok::{Content, Message, Role, StreamEvent};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

#[derive(Clone, Debug)]
pub struct Run {
  pub command: String,
  pub args: Vec<String>,
  pub output: Output,
  pub workspace: PathBuf,
}

pub fn args(args: &[String], model: &str, workspace: &str) -> Vec<String> {
  args.iter().map(|a| a.replace("{model}", model).replace("{workspace}", workspace)).collect()
}

/// The transcript as plain text, role by role.
pub fn render(messages: &[Message]) -> String {
  let mut out = String::new();
  for m in messages {
    let role = match m.role {
      Role::System => "system",
      Role::User => "user",
      Role::Assistant => "assistant",
      Role::Tool => "tool",
    };
    out.push_str(role);
    out.push_str(":\n");
    match &m.content {
      Some(Content::Text(t)) => out.push_str(t),
      Some(c @ Content::Parts(_)) => {
        out.push_str(&c.as_text());
        out.push_str("\n[image content omitted by process provider]");
      }
      None => {}
    }
    out.push('\n');
    for c in m.tool_calls.iter().flatten() {
      out.push_str(&format!("tool call {}: {}\n", c.function.name, c.function.arguments));
    }
    out.push('\n');
  }
  out
}

fn path_with_tools() -> String {
  let path = std::env::var("PATH").unwrap_or_default();
  let home = std::env::var("HOME").unwrap_or_default();
  format!("{home}/.local/bin:/opt/homebrew/bin:/usr/local/bin:{path}")
}

impl Run {
  pub async fn stream(&self, model: &str, messages: &[Message]) -> Result<BoxStream<'static, Result<StreamEvent>>> {
    std::fs::create_dir_all(&self.workspace)?;
    let ws = self.workspace.display().to_string();
    let mut child = tokio::process::Command::new(&self.command)
      .args(args(&self.args, model, &ws))
      .current_dir(&self.workspace)
      .env("PATH", path_with_tools())
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .kill_on_drop(true)
      .spawn()
      .with_context(|| format!("could not start {}", self.command))?;
    let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
    let mut stderr = child.stderr.take().ok_or_else(|| anyhow!("no stderr"))?;
    let prompt = render(messages);
    tokio::spawn(async move {
      let _ = stdin.write_all(prompt.as_bytes()).await;
      let _ = stdin.shutdown().await;
    });
    let (tx, rx) = futures::channel::mpsc::unbounded::<Result<StreamEvent>>();
    let output = self.output;
    let command = self.command.clone();
    tokio::spawn(async move {
      let err = tokio::spawn(async move {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s).await;
        s
      });
      let mut reader = BufReader::new(stdout);
      let mut whole = String::new();
      let mut lines = Lines::default();
      if output == Output::StreamJson {
        let mut line = String::new();
        loop {
          line.clear();
          match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
              for ev in lines.push(&line) {
                let _ = tx.unbounded_send(Ok(ev));
              }
            }
          }
        }
      } else {
        let _ = reader.read_to_string(&mut whole).await;
      }
      let status = child.wait().await;
      let stderr = err.await.unwrap_or_default();
      let failed = !status.as_ref().map(|s| s.success()).unwrap_or(false);
      let result = match output {
        Output::StreamJson => lines.finish(),
        Output::Text => Ok((whole.trim().to_string(), None)),
        Output::JsonResult => serde_json::from_str::<serde_json::Value>(whole.trim())
          .ok()
          .and_then(|v| v["result"].as_str().map(|s| (s.to_string(), None)))
          .ok_or_else(|| anyhow!("{command} returned no result")),
      };
      if failed {
        let detail = if stderr.trim().is_empty() { whole.trim().to_string() } else { stderr.trim().to_string() };
        let _ = tx.unbounded_send(Err(anyhow!("{command} failed: {detail}")));
        return;
      }
      match result {
        Ok((text, usage)) => {
          if (output != Output::StreamJson || !lines.streamed())
            && !text.is_empty() {
              let _ = tx.unbounded_send(Ok(StreamEvent::Text(text)));
            }
          if let Some(u) = usage {
            let _ = tx.unbounded_send(Ok(StreamEvent::Usage(u)));
          }
          let _ = tx.unbounded_send(Ok(StreamEvent::Done { calls: Vec::new(), finish: Some("stop".into()) }));
        }
        Err(e) => {
          let _ = tx.unbounded_send(Err(e));
        }
      }
    });
    Ok(rx.boxed())
  }
}

pub fn check_command(command: &str) -> Result<()> {
  let found = std::process::Command::new("sh")
    .arg("-c")
    .arg(format!("PATH='{}' command -v {}", path_with_tools(), command.replace('\'', "")))
    .output()
    .map(|o| o.status.success())
    .unwrap_or(false);
  if !found {
    bail!("{command} isn't installed or isn't on PATH");
  }
  Ok(())
}

#[cfg(test)]
#[path = "../tests/process.rs"]
mod tests;
