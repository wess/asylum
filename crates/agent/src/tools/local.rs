//! The user's own machine, outside the computer. Gated by the Execution on
//! Local Computer setting and approvals.

use super::{def, need, Ctx, Outcome};
use anyhow::{bail, Result};
use computer::shell::{self, Place, Spec};
use grok::ToolDef;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

pub fn defs() -> Vec<ToolDef> {
  vec![
    def(
      "local_shell",
      "Run a command on the user's own computer (not your computer). Only when the user asks for something on their machine.",
      json!({"type": "object", "properties": {"command": {"type": "string"}, "cwd": {"type": "string"}}, "required": ["command"]}),
    ),
    def("local_read_file", "Read a text file on the user's own computer.", json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]})),
    def(
      "copy_from_local",
      "Copy a file from the user's computer into your workspace.",
      json!({"type": "object", "properties": {"from": {"type": "string", "description": "path on the user's computer"}, "to": {"type": "string", "description": "workspace path"}}, "required": ["from"]}),
    ),
    def(
      "copy_to_local",
      "Copy a workspace file to the user's computer.",
      json!({"type": "object", "properties": {"from": {"type": "string", "description": "workspace path"}, "to": {"type": "string", "description": "path on the user's computer"}}, "required": ["from", "to"]}),
    ),
  ]
}

fn home() -> PathBuf {
  std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

fn expand(p: &str) -> PathBuf {
  match p.strip_prefix("~/") {
    Some(rest) => home().join(rest),
    None if p == "~" => home(),
    None => PathBuf::from(p),
  }
}

pub async fn call(ctx: &Ctx<'_>, name: &str, args: &Value) -> Option<Result<Outcome>> {
  let ws = ctx.rt.computer.workspace();
  let r = match name {
    "local_shell" => match need(args, "command") {
      Ok(command) => {
        let cwd = match args["cwd"].as_str() {
          Some(c) if !c.is_empty() => expand(c),
          _ => home(),
        };
        let spec = Spec { command, cwd: &cwd, place: Place::Local, env: ctx.env, timeout: Duration::from_secs(300), net: ctx.rt.local_net() };
        shell::run(spec).await.map(|o| Outcome::text(o.render()))
      }
      Err(e) => Err(e),
    },
    "local_read_file" => need(args, "path").and_then(|p| {
      let text = std::fs::read_to_string(expand(p))?;
      Ok(Outcome::text(ctx.redact(&computer::clip::clip(&text, 40_000))))
    }),
    "copy_from_local" => need(args, "from").and_then(|f| {
      let src = expand(f);
      if !src.is_file() {
        bail!("{f} is not a file");
      }
      let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
      let to = match args["to"].as_str() {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => format!("from-local/{name}"),
      };
      let bytes = std::fs::read(&src)?;
      let shown = computer::fs::write_bytes(&ws, &to, &bytes)?;
      Ok(Outcome::with(format!("Copied to {shown}"), super::computer::file_part(&ws, &shown)))
    }),
    "copy_to_local" => need(args, "from").and_then(|f| {
      let src = computer::fs::resolve(&ws, f)?;
      let dest = expand(need(args, "to")?);
      if let Some(d) = dest.parent() {
        std::fs::create_dir_all(d)?;
      }
      std::fs::copy(&src, &dest)?;
      Ok(Outcome::text(format!("Copied to {}", dest.display())))
    }),
    _ => return None,
  };
  Some(r)
}
