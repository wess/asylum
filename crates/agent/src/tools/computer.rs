use super::{arg, def, need, Ctx, Outcome};
use crate::part::Part;
use anyhow::Result;
use computer::fs;
use computer::shell::{self, Place, Spec};
use chat::ToolDef;
use serde_json::{json, Value};
use std::time::Duration;

pub fn defs() -> Vec<ToolDef> {
  vec![
    def(
      "shell",
      "Run a shell command on your computer (cwd is the shared workspace). Use for CLIs, scripts, git, package managers, data processing. Secrets are available as env vars.",
      json!({"type": "object", "properties": {
        "command": {"type": "string"},
        "timeout": {"type": "integer", "description": "seconds, default 120, max 1800"}
      }, "required": ["command"]}),
    ),
    def("read_file", "Read a text file from the workspace.", json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]})),
    def(
      "write_file",
      "Create or overwrite a file in the workspace. The user sees it as a file card.",
      json!({"type": "object", "properties": {"path": {"type": "string"}, "content": {"type": "string"}}, "required": ["path", "content"]}),
    ),
    def(
      "edit_file",
      "Replace one exact occurrence of text in a workspace file.",
      json!({"type": "object", "properties": {"path": {"type": "string"}, "old": {"type": "string"}, "new": {"type": "string"}}, "required": ["path", "old", "new"]}),
    ),
    def("list_files", "List a workspace directory.", json!({"type": "object", "properties": {"path": {"type": "string", "description": "default: workspace root"}}})),
    def(
      "search_files",
      "Find workspace files whose name or contents contain text.",
      json!({"type": "object", "properties": {"query": {"type": "string"}, "path": {"type": "string"}}, "required": ["query"]}),
    ),
    def("delete_file", "Delete a workspace file or folder.", json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]})),
    def(
      "move_file",
      "Move or rename a workspace file or folder.",
      json!({"type": "object", "properties": {"from": {"type": "string"}, "to": {"type": "string"}}, "required": ["from", "to"]}),
    ),
  ]
}

pub fn mime(path: &str) -> String {
  let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
  match ext.as_str() {
    "png" => "image/png",
    "jpg" | "jpeg" => "image/jpeg",
    "gif" => "image/gif",
    "webp" => "image/webp",
    "svg" => "image/svg+xml",
    "pdf" => "application/pdf",
    "md" | "markdown" => "text/markdown",
    "csv" => "text/csv",
    "json" => "application/json",
    "yaml" | "yml" => "application/yaml",
    "html" | "htm" => "text/html",
    "txt" | "log" => "text/plain",
    "mp3" => "audio/mpeg",
    "wav" => "audio/wav",
    "m4a" => "audio/mp4",
    "aiff" | "aif" => "audio/aiff",
    "mp4" => "video/mp4",
    "mov" => "video/quicktime",
    "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "ipynb" => "application/x-ipynb+json",
    "eml" => "message/rfc822",
    _ => "application/octet-stream",
  }
  .to_string()
}

pub fn file_part(ws: &std::path::Path, shown: &str) -> Part {
  let rel = shown.trim_start_matches("~/");
  let path = ws.join(rel);
  let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
  let m = mime(&name);
  if m.starts_with("image/") {
    Part::Image { path: path.display().to_string(), caption: name }
  } else {
    Part::File { path: path.display().to_string(), name, mime: m }
  }
}

pub async fn call(ctx: &Ctx<'_>, name: &str, args: &Value) -> Option<Result<Outcome>> {
  let ws = ctx.rt.computer.workspace();
  let r = match name {
    "shell" => {
      let command = match need(args, "command") {
        Ok(c) => c,
        Err(e) => return Some(Err(e)),
      };
      let secs = args["timeout"].as_u64().unwrap_or(120).clamp(1, 1800);
      let place = if ctx.rt.settings().isolate_computer { Place::Computer } else { Place::Local };
      let spec = Spec { command, cwd: &ws, place, env: ctx.env, timeout: Duration::from_secs(secs), net: if place == computer::shell::Place::Local { ctx.rt.local_net() } else { ctx.rt.net() } };
      shell::run(spec).await.map(|o| Outcome::text(o.render()))
    }
    "read_file" => need(args, "path")
      .and_then(|p| fs::read(&ws, p))
      .map(|t| Outcome::text(ctx.redact(&computer::clip::clip(&t, 40_000)))),
    "write_file" => need(args, "path").and_then(|p| {
      let shown = fs::write(&ws, p, arg(args, "content"))?;
      Ok(Outcome::with(format!("Wrote {shown}"), file_part(&ws, &shown)))
    }),
    "edit_file" => need(args, "path").and_then(|p| {
      fs::edit(&ws, p, arg(args, "old"), arg(args, "new"))?;
      Ok(Outcome::text(format!("Edited {p}")))
    }),
    "list_files" => {
      let dir = match arg(args, "path") {
        "" => "~",
        d => d,
      };
      fs::list(&ws, dir).map(|entries| {
        if entries.is_empty() {
          return Outcome::text("(empty)");
        }
        let lines: Vec<String> = entries
          .iter()
          .map(|e| if e.dir { format!("{}/", e.path) } else { format!("{} ({} bytes)", e.path, e.size) })
          .collect();
        Outcome::text(lines.join("\n"))
      })
    }
    "search_files" => need(args, "query").and_then(|q| {
      let dir = match arg(args, "path") {
        "" => "~",
        d => d,
      };
      let hits = fs::search(&ws, dir, q, 50)?;
      Ok(Outcome::text(if hits.is_empty() { "No matches.".into() } else { hits.join("\n") }))
    }),
    "delete_file" => need(args, "path").and_then(|p| {
      fs::remove(&ws, p)?;
      Ok(Outcome::text(format!("Deleted {p}")))
    }),
    "move_file" => need(args, "from").and_then(|f| {
      let to = need(args, "to")?;
      fs::rename(&ws, f, to)?;
      Ok(Outcome::text(format!("Moved {f} to {to}")))
    }),
    _ => return None,
  };
  Some(r)
}
