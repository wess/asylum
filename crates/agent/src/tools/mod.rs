//! The tools a Bot can call. Each module offers definitions and handles its
//! own names; `call` routes by name and plugin tools fall through to the
//! hub. `class` and `target` feed the approval gate.

pub mod browser;
pub mod computer;
pub mod local;
pub mod media;
pub mod memory;
pub mod routines;
pub mod sheet;
pub mod skills;
pub mod team;
pub mod user;
pub mod web;

use crate::approve::Class;
use crate::part::Part;
use crate::plugins::Offered;
use crate::queue::Origin;
use crate::runtime::Runtime;
use anyhow::{anyhow, Result};
use chat::ToolDef;
use serde_json::Value;
pub use sheet::Sheet;
use store::{Bot, Chat};
use tokio_util::sync::CancellationToken;

pub struct Ctx<'a> {
  pub rt: &'a Runtime,
  pub bot: &'a Bot,
  pub chat: &'a Chat,
  pub run: &'a str,
  pub origin: Origin,
  pub cancel: &'a CancellationToken,
  pub sheet: &'a Sheet,
  /// Secret env vars for this Bot (name, value).
  pub env: &'a [(String, String)],
}

impl Ctx<'_> {
  pub fn secrets(&self) -> Vec<String> {
    self.env.iter().map(|(_, v)| v.clone()).collect()
  }

  pub fn redact(&self, text: &str) -> String {
    ::computer::redact::redact(text, &self.secrets())
  }
}

#[derive(Default)]
pub struct Outcome {
  pub text: String,
  /// Cards to attach to the Bot's message.
  pub parts: Vec<Part>,
  /// End the turn after this tool (the Bot is waiting on the user).
  pub stop: bool,
}

impl Outcome {
  pub fn text(t: impl Into<String>) -> Self {
    Self { text: t.into(), ..Default::default() }
  }

  pub fn with(t: impl Into<String>, part: Part) -> Self {
    Self { text: t.into(), parts: vec![part], stop: false }
  }
}

pub fn arg<'v>(args: &'v Value, key: &str) -> &'v str {
  args.get(key).and_then(Value::as_str).unwrap_or("")
}

pub fn need<'v>(args: &'v Value, key: &str) -> Result<&'v str> {
  args
    .get(key)
    .and_then(Value::as_str)
    .filter(|s| !s.trim().is_empty())
    .ok_or_else(|| anyhow!("missing \"{key}\""))
}

pub fn def(name: &str, description: &str, params: Value) -> ToolDef {
  ToolDef::new(name, description, params)
}

/// Built-in tools for this Bot and chat.
pub fn builtin(chat: &Chat, voice: bool) -> Vec<ToolDef> {
  let mut all = Vec::new();
  all.extend(computer::defs());
  all.extend(web::defs());
  all.extend(browser::defs());
  all.extend(local::defs());
  all.extend(memory::defs());
  all.extend(skills::defs());
  all.extend(routines::defs());
  all.extend(team::defs(chat));
  all.extend(user::defs());
  all.extend(media::defs(voice));
  all
}

pub fn class(name: &str, offered: &[Offered]) -> Class {
  if let Some(o) = offered.iter().find(|o| o.name == name) {
    return crate::approve::plugin_class(&o.tool.name, o.tool.read_only());
  }
  match name {
    "shell" | "write_file" | "edit_file" | "move_file" | "browser_open" | "browser_click" | "browser_type"
    | "browser_press" => Class::Review,
    "delete_file" | "create_routine" | "update_routine" | "delete_routine" | "set_routine_active" | "create_agent"
    | "test_routine" => Class::Consequential,
    "local_shell" | "local_read_file" | "copy_to_local" | "copy_from_local" => Class::Local,
    _ => Class::Free,
  }
}

/// What an action touches, shown on its approval card and used to scope
/// "Always allow".
pub fn target(name: &str, args: &Value) -> String {
  let pick = |k: &str| arg(args, k).to_string();
  match name {
    "shell" | "local_shell" => pick("command"),
    "write_file" | "edit_file" | "delete_file" | "local_read_file" => pick("path"),
    "move_file" => format!("{} → {}", arg(args, "from"), arg(args, "to")),
    "copy_to_local" | "copy_from_local" => format!("{} → {}", arg(args, "from"), arg(args, "to")),
    "browser_open" => pick("url"),
    "create_routine" | "update_routine" | "delete_routine" | "set_routine_active" | "test_routine" => pick("name"),
    "create_agent" => pick("name"),
    _ => String::new(),
  }
}

pub async fn call(ctx: &Ctx<'_>, name: &str, args: &Value, offered: &[Offered]) -> Result<Outcome> {
  if let Some(r) = computer::call(ctx, name, args).await {
    return r;
  }
  if let Some(r) = web::call(ctx, name, args).await {
    return r;
  }
  if let Some(r) = browser::call(ctx, name, args).await {
    return r;
  }
  if let Some(r) = local::call(ctx, name, args).await {
    return r;
  }
  if let Some(r) = memory::call(ctx, name, args).await {
    return r;
  }
  if let Some(r) = skills::call(ctx, name, args).await {
    return r;
  }
  if let Some(r) = routines::call(ctx, name, args).await {
    return r;
  }
  if let Some(r) = team::call(ctx, name, args).await {
    return r;
  }
  if let Some(r) = user::call(ctx, name, args).await {
    return r;
  }
  if let Some(r) = media::call(ctx, name, args).await {
    return r;
  }
  if let Some(o) = offered.iter().find(|o| o.name == name) {
    let (text, is_error) = ctx.rt.plugins.call(ctx.rt, o, args.clone()).await?;
    let text = ctx.redact(&::computer::clip::clip(&text, 30_000));
    return Ok(Outcome::text(if is_error { format!("[error] {text}") } else { text }));
  }
  Err(anyhow!("unknown tool {name}"))
}
