//! Computer use in the Bot's own browser screen.

use super::{arg, def, need, Ctx, Outcome};
use crate::event::Event;
use crate::part::Part;
use crate::runtime::Reply;
use anyhow::Result;
use grok::ToolDef;
use serde_json::{json, Value};

pub fn defs() -> Vec<ToolDef> {
  let index = json!({"type": "integer", "description": "the element number from the last look"});
  vec![
    def("browser_open", "Open a URL in your browser screen. Returns the page text and numbered interactive elements.", json!({"type": "object", "properties": {"url": {"type": "string"}}, "required": ["url"]})),
    def("browser_look", "Look at your browser screen again: page text and numbered interactive elements.", json!({"type": "object", "properties": {}})),
    def("browser_click", "Click an element by number.", json!({"type": "object", "properties": {"index": index}, "required": ["index"]})),
    def(
      "browser_type",
      "Type into a field by number (replaces its contents). Set submit to press Enter after.",
      json!({"type": "object", "properties": {"index": index, "text": {"type": "string"}, "submit": {"type": "boolean"}}, "required": ["index", "text"]}),
    ),
    def(
      "browser_press",
      "Press a key: Enter, Tab, Escape, Backspace, ArrowUp/Down/Left/Right, PageUp/PageDown, Home, End, Space.",
      json!({"type": "object", "properties": {"key": {"type": "string"}}, "required": ["key"]}),
    ),
    def("browser_scroll", "Scroll the page by pixels (negative is up).", json!({"type": "object", "properties": {"dy": {"type": "integer"}}, "required": ["dy"]})),
    def("browser_back", "Go back in browser history.", json!({"type": "object", "properties": {}})),
    def("browser_screenshot", "Capture your screen as an image the user can see.", json!({"type": "object", "properties": {}})),
    def(
      "request_takeover",
      "Ask the user to take over your screen for something only they should do: passwords, passkeys, 2FA, CAPTCHA, payment, identity checks. Waits until they finish or skip. You never see what they type.",
      json!({"type": "object", "properties": {"reason": {"type": "string"}}, "required": ["reason"]}),
    ),
  ]
}

fn index(args: &Value) -> u32 {
  args["index"].as_u64().unwrap_or(0) as u32
}

pub async fn call(ctx: &Ctx<'_>, name: &str, args: &Value) -> Option<Result<Outcome>> {
  let b = &ctx.rt.browser;
  let bot = ctx.bot.id.as_str();
  let snap = |r: Result<computer::browser::Snapshot>| {
    r.map(|s| Outcome::text(format!("[untrusted web content]\n{}", ctx.redact(&s.render()))))
  };
  let r = match name {
    "browser_open" => match need(args, "url") {
      Ok(u) => snap(b.open(bot, u).await),
      Err(e) => Err(e),
    },
    "browser_look" => snap(b.snapshot(bot).await),
    "browser_click" => snap(b.click(bot, index(args)).await),
    "browser_type" => snap(b.type_text(bot, index(args), arg(args, "text"), args["submit"].as_bool().unwrap_or(false)).await),
    "browser_press" => match need(args, "key") {
      Ok(k) => snap(b.press(bot, k).await),
      Err(e) => Err(e),
    },
    "browser_scroll" => snap(b.scroll(bot, args["dy"].as_i64().unwrap_or(600)).await),
    "browser_back" => snap(b.back(bot).await),
    "browser_screenshot" => screenshot(ctx).await,
    "request_takeover" => takeover(ctx, arg(args, "reason")).await,
    _ => return None,
  };
  if name.starts_with("browser_") {
    ctx.rt.emit(Event::Screen { bot: bot.to_string() });
  }
  Some(r)
}

async fn screenshot(ctx: &Ctx<'_>) -> Result<Outcome> {
  let png = ctx.rt.browser.screenshot(&ctx.bot.id).await?;
  let dir = ctx.rt.computer.screens(&ctx.bot.id);
  std::fs::create_dir_all(&dir)?;
  let path = dir.join(format!("{}.png", store::now()));
  std::fs::write(&path, png)?;
  Ok(Outcome::with("Captured the screen.", Part::Image { path: path.display().to_string(), caption: "Screen".into() }))
}

async fn takeover(ctx: &Ctx<'_>, reason: &str) -> Result<Outcome> {
  let i = ctx.sheet.push(Part::Takeover { reason: reason.to_string(), status: "pending".into() });
  ctx.sheet.save(ctx.rt).await?;
  let key = format!("takeover:{}", ctx.bot.id);
  let rx = ctx.rt.wait(&key).await;
  crate::turn::attention(ctx.rt, ctx.bot, ctx.chat, "Action needed", reason).await;
  let (status, text) = tokio::select! {
    r = rx => match r {
      Ok(Reply::Takeover(true)) => ("done", "The user finished on your screen. Take a fresh look and continue."),
      _ => ("skipped", "The user skipped this. Continue without it or explain what is blocked."),
    },
    _ = ctx.cancel.cancelled() => ("skipped", "Stopped."),
  };
  ctx.sheet.set(i, Part::Takeover { reason: reason.to_string(), status: status.into() });
  crate::turn::resume(ctx.rt, ctx.bot, ctx.chat).await;
  Ok(Outcome::text(text))
}
