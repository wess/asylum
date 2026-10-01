//! Bots working together: the roster, asynchronous handoffs, helper Bots,
//! and group chats.

use super::{arg, def, need, Ctx, Outcome};
use crate::event::Event;
use crate::part::Part;
use crate::queue::{Job, Origin};
use anyhow::{anyhow, bail, Result};
use grok::ToolDef;
use serde_json::{json, Value};
use store::{bots, chats, messages};

pub const HANDOFF_LIMIT: usize = 3;
pub const HANDOFF_WINDOW_MS: i64 = 10 * 60 * 1000;

pub fn defs(chat: &store::Chat) -> Vec<ToolDef> {
  let mut v = vec![
    def("list_bots", "List every Bot on the team with their job and whether they are busy.", json!({"type": "object", "properties": {}})),
    def(
      "message_bot",
      "Send an asynchronous message to another Bot. It wakes, handles it in its own chat, and can reply to you later the same way. Set transfer to hand over ownership of the task. You can share workspace images by path.",
      json!({"type": "object", "properties": {
        "bot": {"type": "string", "description": "the Bot's name"},
        "message": {"type": "string"},
        "transfer": {"type": "boolean"},
        "images": {"type": "array", "items": {"type": "string"}}
      }, "required": ["bot", "message"]}),
    ),
    def(
      "create_bot",
      "Create a focused helper Bot with a name, a one-line job, and standing instructions. Ask the user first if they prefer a small roster.",
      json!({"type": "object", "properties": {
        "name": {"type": "string"}, "label": {"type": "string"}, "description": {"type": "string"}
      }, "required": ["name", "label", "description"]}),
    ),
    def(
      "create_group",
      "Start a group chat with 2 to 6 Bots (you are added automatically).",
      json!({"type": "object", "properties": {
        "name": {"type": "string"}, "bots": {"type": "array", "items": {"type": "string"}}, "description": {"type": "string"}
      }, "required": ["bots"]}),
    ),
  ];
  if !chat.is_group() {
    v.push(def(
      "post_to_group",
      "Post a text-only message to a group chat you belong to.",
      json!({"type": "object", "properties": {"group": {"type": "string"}, "message": {"type": "string"}}, "required": ["group", "message"]}),
    ));
  }
  v
}

pub async fn call(ctx: &Ctx<'_>, name: &str, args: &Value) -> Option<Result<Outcome>> {
  let r = match name {
    "list_bots" => roster(ctx).await,
    "message_bot" => handoff(ctx, args).await,
    "create_bot" => helper(ctx, args).await,
    "create_group" => group(ctx, args).await,
    "post_to_group" => post(ctx, args).await,
    _ => return None,
  };
  Some(r)
}

async fn roster(ctx: &Ctx<'_>) -> Result<Outcome> {
  let all = bots::list(&ctx.rt.pool).await?;
  let lines: Vec<String> = all
    .iter()
    .filter(|b| b.kind != bots::SYSTEM)
    .map(|b| {
      let me = if b.id == ctx.bot.id { " (you)" } else { "" };
      let busy = if ctx.rt.queues.busy(&b.id) { "working" } else { "free" };
      format!("- {}{me} — {} [{busy}]{}", b.name, if b.label.is_empty() { "no job set" } else { &b.label }, if b.hidden { " (hidden)" } else { "" })
    })
    .collect();
  Ok(Outcome::text(lines.join("\n")))
}

async fn handoff(ctx: &Ctx<'_>, args: &Value) -> Result<Outcome> {
  let pool = &ctx.rt.pool;
  let to_name = need(args, "bot")?;
  let to = bots::find(pool, to_name).await?.ok_or_else(|| anyhow!("no Bot named {to_name}"))?;
  if to.id == ctx.bot.id {
    bail!("that's you");
  }
  let text = need(args, "message")?;
  let transfer = args["transfer"].as_bool().unwrap_or(false);
  let chat = chats::direct(pool, &to.id).await?;
  // Stop Bots from bouncing thanks back and forth: a few handoffs to the
  // same Bot per window, reset whenever the user speaks in that chat.
  let since = store::now() - HANDOFF_WINDOW_MS;
  let recent = messages::recent(pool, &chat.id, 40).await?;
  let last_user = recent.iter().rev().find(|m| m.role == messages::USER).map(|m| m.created).unwrap_or(0);
  let sent = recent
    .iter()
    .filter(|m| m.bot_id.as_deref() == Some(ctx.bot.id.as_str()) && m.created > since.max(last_user))
    .count();
  if sent >= HANDOFF_LIMIT {
    return Ok(Outcome::text(format!(
      "Not sent: you've already sent {} {HANDOFF_LIMIT} messages recently. Don't send acknowledgements or thanks; continue on your own or tell the user.",
      to.name
    )));
  }
  let mut parts = vec![Part::Handoff { from: ctx.bot.name.clone(), to: to.name.clone(), direction: if transfer { "transfer".into() } else { "in".into() } }];
  let ws = ctx.rt.computer.workspace();
  for img in args["images"].as_array().into_iter().flatten().filter_map(Value::as_str) {
    if let Ok(p) = computer::fs::resolve(&ws, img) {
      parts.push(Part::Image { path: p.display().to_string(), caption: img.to_string() });
    }
  }
  let body = if transfer { format!("{text}\n\n(You now own this task.)") } else { text.to_string() };
  let m = messages::add(pool, messages::New {
    chat: &chat.id,
    bot: Some(&ctx.bot.id),
    role: messages::BOT,
    body: &body,
    parts: &crate::part::to_values(&parts),
    status: messages::DONE,
    run: None,
    thread: None,
  })
  .await?;
  ctx.rt.emit(Event::Message { chat: chat.id.clone(), message: m.id });
  crate::turn::enqueue(ctx.rt, Job {
    bot: to.id.clone(),
    chat: chat.id,
    origin: Origin::Handoff,
    routine: None,
    note: format!(
      "{} sent you this. Do the work. Use message_bot to reply only when you have a result or a real question for them — never just to thank or acknowledge.",
      ctx.bot.name
    ),
  });
  Ok(Outcome::with(
    format!("Sent to {}. They'll work on it in their chat{}.", to.name, if transfer { " and now own it" } else { "" }),
    Part::Handoff { from: ctx.bot.name.clone(), to: to.name, direction: "out".into() },
  ))
}

async fn helper(ctx: &Ctx<'_>, args: &Value) -> Result<Outcome> {
  let pool = &ctx.rt.pool;
  if bots::count(pool).await? >= crate::turn::ROSTER_LIMIT {
    bail!("the roster is full ({} Bots and groups)", crate::turn::ROSTER_LIMIT);
  }
  let base = need(args, "name")?;
  let name = bots::unique_name(pool, base).await?;
  let p = bots::Profile {
    name,
    label: arg(args, "label").into(),
    description: arg(args, "description").into(),
    avatar: String::new(),
    color: String::new(),
  };
  let b = bots::create_kind(pool, &p, bots::HELPER, Some(&ctx.bot.id)).await?;
  chats::direct(pool, &b.id).await?;
  ctx.rt.emit(Event::BotsChanged);
  crate::turn::notify(ctx.rt, &b, None, &format!("{} created {}", ctx.bot.name, b.name), &b.label).await;
  Ok(Outcome::text(format!("Created {} ({}). Reach them with message_bot.", b.name, b.label)))
}

async fn group(ctx: &Ctx<'_>, args: &Value) -> Result<Outcome> {
  let pool = &ctx.rt.pool;
  let mut ids = vec![ctx.bot.id.clone()];
  for n in args["bots"].as_array().into_iter().flatten().filter_map(Value::as_str) {
    let b = bots::find(pool, n).await?.ok_or_else(|| anyhow!("no Bot named {n}"))?;
    if !ids.contains(&b.id) {
      ids.push(b.id);
    }
  }
  if !(2..=6).contains(&ids.len()) {
    bail!("a group has 2 to 6 Bots");
  }
  let title = match arg(args, "name") {
    "" => crate::turn::group_name(ctx.rt, &ids).await,
    n => n.to_string(),
  };
  let g = chats::create_group(pool, &title, &ids).await?;
  chats::set_description(pool, &g.id, arg(args, "description")).await?;
  ctx.rt.emit(Event::ChatsChanged);
  Ok(Outcome::text(format!("Started group \"{title}\".")))
}

async fn post(ctx: &Ctx<'_>, args: &Value) -> Result<Outcome> {
  let pool = &ctx.rt.pool;
  let want = need(args, "group")?.to_lowercase();
  let mine = chats::involving(pool, &ctx.bot.id).await?;
  let g = mine
    .into_iter()
    .find(|c| c.is_group() && c.title.to_lowercase() == want)
    .ok_or_else(|| anyhow!("you are not in a group named {want}"))?;
  let m = messages::add(pool, messages::New {
    chat: &g.id,
    bot: Some(&ctx.bot.id),
    role: messages::BOT,
    body: need(args, "message")?,
    parts: &[],
    status: messages::DONE,
    run: None,
    thread: None,
  })
  .await?;
  chats::set_unread(pool, &g.id, true).await?;
  ctx.rt.emit(Event::Message { chat: g.id.clone(), message: m.id });
  ctx.rt.emit(Event::ChatsChanged);
  Ok(Outcome::text(format!("Posted to {}.", g.title)))
}
