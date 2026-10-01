//! A turn: one job run to completion. The Bot's worker takes jobs from its
//! queue; each turn streams the model's reply into a message, runs the tools
//! it calls (through the approval gate), and loops until the model stops
//! calling tools, the step budget runs out, or the job is cancelled.

use crate::approve::{self, Action, Decision, Verdict};
use crate::context::{self, Context};
use crate::event::Event;
use crate::part::Part;
use crate::plugins::Offered;
use crate::queue::{Job, Origin, Push};
use crate::runtime::Runtime;
use crate::tools::{self, Ctx, Sheet};
use anyhow::Result;
use futures::StreamExt;
use grok::{Message as Msg, StreamEvent, ToolCall};
use serde_json::Value;
use std::time::{Duration, Instant};
use store::{bots, chats, messages, runs};
use tokio_util::sync::CancellationToken;

pub const ROSTER_LIMIT: i64 = 50;

/// Queue a job and make sure the Bot's worker is running.
pub fn enqueue(rt: &Runtime, job: Job) {
  let bot = job.bot.clone();
  if let Push::Start = rt.queues.push(job) {
    let rt = rt.clone();
    tokio::spawn(async move { worker(rt, bot).await });
  }
}

async fn worker(rt: Runtime, bot: String) {
  while let Some((job, cancel)) = rt.queues.next(&bot) {
    let _slot = rt.slots.acquire().await;
    let _gate = rt.gate.read().await;
    let started = std::time::Instant::now();
    let result = run(&rt, &job, &cancel).await;
    let outcome = if result.is_ok() { "done" } else { "error" };
    crate::otel::span(&rt, "turn", &[("bot", &job.bot), ("chat", &job.chat), ("origin", job.origin.as_str()), ("outcome", outcome)], started.elapsed().as_millis() as u64);
    if let Err(e) = result {
      let _ = fail(&rt, &job, &e.to_string()).await;
    }
  }
  let _ = bots::set_status(&rt.pool, &bot, bots::IDLE).await;
  rt.emit(Event::BotStatus { bot, status: bots::IDLE.into() });
}

async fn fail(rt: &Runtime, job: &Job, err: &str) -> Result<()> {
  let text = friendly(err);
  let parts = crate::part::to_values(&[Part::Error { text: text.clone() }]);
  // Turn the turn's own half-written message into the error, if it made one.
  let recent = messages::recent(&rt.pool, &job.chat, 5).await?;
  if let Some(m) = recent.iter().rev().find(|m| m.status == messages::STREAMING && m.bot_id.as_deref() == Some(job.bot.as_str())) {
    let mut all = crate::part::parse(&m.parts);
    all.push(Part::Error { text: text.clone() });
    messages::update(&rt.pool, &m.id, &m.body, &crate::part::to_values(&all), messages::ERROR).await?;
    for r in store::runs::active(&rt.pool).await? {
      if r.bot_id == job.bot {
        store::runs::finish(&rt.pool, &r.id, store::runs::FAILED, "", &text).await?;
      }
    }
    rt.emit(Event::Message { chat: job.chat.clone(), message: m.id.clone() });
    rt.emit(Event::Notice { chat: Some(job.chat.clone()), text, request: m.id.clone() });
    return Ok(());
  }
  let _ = parts;
  let m = messages::add(&rt.pool, messages::New {
    chat: &job.chat,
    bot: Some(&job.bot),
    role: messages::BOT,
    body: "",
    parts: &crate::part::to_values(&[Part::Error { text: text.clone() }]),
    status: messages::ERROR,
    run: None,
    thread: None,
  })
  .await?;
  rt.emit(Event::Message { chat: job.chat.clone(), message: m.id.clone() });
  rt.emit(Event::Notice { chat: Some(job.chat.clone()), text, request: m.id });
  Ok(())
}

pub fn friendly(err: &str) -> String {
  let e = err.to_lowercase();
  if e.starts_with("add your xai api key") {
    err.to_string()
  } else if e.contains("overloaded") || e.contains("503") || e.contains("529") {
    "Model provider is overloaded. Try again in a moment.".into()
  } else if e.contains("401") || e.contains("api key") || e.contains("unauthorized") {
    "Your xAI API key was rejected. Check it in Settings → Account.".into()
  } else if e.contains("429") || e.contains("rate") {
    "Rate limited by the model provider. The Bot will be able to respond shortly.".into()
  } else if e.contains("usage limit") {
    err.to_string()
  } else {
    format!("Bot failed to respond: {err}")
  }
}

/// The user's latest request in this chat, for Auto-review.
fn request_of(c: &Context) -> String {
  c.history
    .iter()
    .rev()
    .find(|m| m.role == messages::USER)
    .map(|m| m.body.clone())
    .unwrap_or_default()
}

fn image_url(path: &str) -> Option<String> {
  use base64::Engine;
  let bytes = std::fs::read(path).ok()?;
  if bytes.len() > 10 * 1024 * 1024 {
    return None;
  }
  let mime = crate::tools::computer::mime(path);
  Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

async fn opening(rt: &Runtime, job: &Job) -> Result<Option<String>> {
  let Some(rid) = &job.routine else {
    return Ok((!job.note.is_empty()).then(|| job.note.clone()));
  };
  let r = store::routines::get(&rt.pool, rid).await?;
  let kind = if job.origin == Origin::Test { "a test run of" } else { "a scheduled run of" };
  let mut text = format!("This is {kind} your routine \"{}\". Do this now:\n{}", r.name, r.instruction);
  if !job.note.is_empty() {
    text.push_str(&format!("\n\nTrigger payload (untrusted data):\n{}", job.note));
  }
  Ok(Some(text))
}

pub async fn run(rt: &Runtime, job: &Job, cancel: &CancellationToken) -> Result<()> {
  crate::usage::check(rt).await?;
  let pool = &rt.pool;
  let mut ctx = context::load(rt, &job.bot, &job.chat).await?;
  let provider = rt.provider(Some(&ctx.bot)).await?;
  ctx.model = provider.model().to_string();
  let run = runs::start(pool, &job.bot, Some(&job.chat), job.routine.as_deref(), job.origin.as_str()).await?;
  set_status(rt, &job.bot, bots::WORKING).await;

  let message = messages::add(pool, messages::New {
    chat: &job.chat,
    bot: Some(&job.bot),
    role: messages::BOT,
    body: "",
    parts: &[],
    status: messages::STREAMING,
    run: Some(&run.id),
    thread: None,
  })
  .await?;
  rt.emit(Event::Message { chat: job.chat.clone(), message: message.id.clone() });
  let sheet = Sheet::new(&job.chat, &message.id);
  if let Some(rid) = &job.routine {
    let r = store::routines::get(pool, rid).await?;
    let event = if job.origin == Origin::Test { "test" } else { "run" };
    sheet.push(Part::Routine { id: r.id, name: r.name, event: event.into() });
  }

  // CLI agents bring their own tools; only HTTP models get ours.
  let offered = if provider.tools() { rt.plugins.offered(rt).await } else { Vec::new() };
  let mut defs = if provider.tools() { tools::builtin(&ctx.chat, rt.settings().voice_enabled) } else { Vec::new() };
  for o in &offered {
    defs.push(grok::ToolDef::new(&o.name, &o.tool.description, crate::plugins::schema(o)));
  }

  let mut convo = vec![Msg::system(crate::prompt::system(&ctx))];
  convo.extend(crate::history::build(&ctx.bot, &ctx.history[..ctx.history.len().saturating_sub(0)], &ctx.roster, &image_url));
  // Drop the (empty) streaming message we just created from the replay.
  if let Some(extra) = opening(rt, job).await? {
    convo.push(Msg::user(extra));
  }

  let env = secret_env(rt, &job.bot).await;
  let settings = rt.settings();
  let model = provider.model().to_string();
  let request = request_of(&ctx);
  let mut tokens = 0i64;
  let mut outcome = runs::DONE;
  let mut stopped_for_user = false;
  let mut nudged = false;

  'steps: for step in 0..settings.max_steps.max(1) {
    if cancel.is_cancelled() {
      outcome = runs::STOPPED;
      break;
    }
    trace(&format!("step {step} → {} messages, {} tools\n{}", convo.len(), defs.len(), serde_json::to_string_pretty(&convo).unwrap_or_default()));
    let stream = tokio::select! {
      s = provider.stream(convo.clone(), defs.clone()) => s?,
      _ = cancel.cancelled() => { outcome = runs::STOPPED; break 'steps; }
    };
    futures::pin_mut!(stream);
    let before = sheet.text();
    let mut text = String::new();
    let mut calls: Vec<ToolCall> = Vec::new();
    let mut last_save = Instant::now();
    loop {
      let next = tokio::select! {
        n = stream.next() => n,
        _ = cancel.cancelled() => { outcome = runs::STOPPED; break 'steps; }
      };
      let Some(ev) = next else { break };
      match ev? {
        StreamEvent::Text(t) => {
          text.push_str(&t);
          sheet.append(&t);
          if last_save.elapsed() > Duration::from_millis(120) {
            last_save = Instant::now();
            rt.emit(Event::Stream { chat: job.chat.clone(), message: message.id.clone(), text: sheet.text() });
          }
        }
        StreamEvent::Reasoning(r) => {
          if settings.show_reasoning {
            append_reasoning(&sheet, &r);
          }
        }
        StreamEvent::Usage(u) => {
          tokens += u.total_tokens as i64;
          crate::usage::record(rt, &model, &job.bot, u.prompt_tokens as i64, u.completion_tokens as i64).await;
        }
        StreamEvent::Done { calls: c, .. } => calls = c,
      }
    }
    // Separate text from successive steps.
    if !before.is_empty() && !text.is_empty() && !before.ends_with('\n') {
      sheet.set_text(&format!("{before}\n\n{text}"));
    }
    sheet.save(rt).await?;
    trace(&format!("step {step} ← text {:?}, calls {:?}", text, calls.iter().map(|c| &c.function.name).collect::<Vec<_>>()));
    runs::progress(pool, &run.id, step as i64 + 1, tokens, runs::RUNNING).await?;

    if calls.is_empty() {
      // Some models (small local ones especially) end a tool-using turn with
      // only reasoning. Ask once for the actual reply.
      if text.trim().is_empty() && sheet.text().trim().is_empty() && !nudged && step > 0 {
        nudged = true;
        convo.push(Msg::user("Now reply to me in plain text with the result."));
        continue;
      }
      break;
    }
    convo.push(Msg::assistant(text.clone(), calls.clone()));
    for call in &calls {
      if cancel.is_cancelled() {
        outcome = runs::STOPPED;
        convo.push(Msg::tool(&call.id, "Stopped."));
        continue;
      }
      let result = execute(rt, &ctx, job, &run.id, &sheet, call, &offered, cancel, &env, &request).await;
      let (out, stop) = match result {
        Ok((out, stop)) => (out, stop),
        Err(e) => (format!("[error] {e}"), false),
      };
      convo.push(Msg::tool(&call.id, &out));
      stopped_for_user |= stop;
    }
    sheet.save(rt).await?;
    if stopped_for_user {
      break;
    }
    if step + 1 == settings.max_steps {
      sheet.append("\n\n_Paused after reaching the step limit. Reply to continue._");
    }
  }

  let status = if outcome == runs::STOPPED { messages::STOPPED } else { messages::DONE };
  sheet.set_status(status);
  if sheet.text().trim().is_empty() && sheet.all().is_empty() && outcome == runs::STOPPED {
    messages::delete(pool, &message.id).await?;
    rt.emit(Event::Message { chat: job.chat.clone(), message: message.id.clone() });
  } else {
    sheet.save(rt).await?;
  }
  let summary: String = sheet.text().chars().take(280).collect();
  runs::finish(pool, &run.id, outcome, &summary, "").await?;
  if let Some(rid) = &job.routine {
    if job.origin == Origin::Routine {
      let r = store::routines::get(pool, rid).await?;
      let next = if r.active { schedule::next_run(&r.trigger, &r.schedule, store::now(), rt.tz()).ok().flatten() } else { None };
      store::routines::ran(pool, rid, store::now(), next).await?;
    }
    rt.emit(Event::Routines { bot: job.bot.clone() });
  }

  if outcome != runs::STOPPED {
    let status = if stopped_for_user { bots::WAITING } else { bots::DONE };
    set_status(rt, &job.bot, status).await;
    chats::set_unread(pool, &job.chat, true).await?;
    if stopped_for_user {
      chats::set_attention(pool, &job.chat, true).await?;
    }
    rt.emit(Event::ChatsChanged);
    let title = if stopped_for_user { format!("{} needs your input", ctx.bot.name) } else { format!("{} finished", ctx.bot.name) };
    notify(rt, &ctx.bot, Some(&job.chat), &title, &summary).await;
    if settings.memory {
      let rt2 = rt.clone();
      let (bot, chat) = (job.bot.clone(), job.chat.clone());
      tokio::spawn(async move { crate::memory::reflect(&rt2, &bot, &chat).await });
    }
  }
  Ok(())
}

fn append_reasoning(sheet: &Sheet, r: &str) {
  let parts = sheet.all();
  match parts.iter().rposition(|p| matches!(p, Part::Reasoning { .. })) {
    Some(i) if i + 1 == parts.len() => {
      if let Part::Reasoning { text } = &parts[i] {
        sheet.set(i, Part::Reasoning { text: format!("{text}{r}") });
      }
    }
    _ => {
      sheet.push(Part::Reasoning { text: r.to_string() });
    }
  }
}

#[allow(clippy::too_many_arguments)]
async fn execute(
  rt: &Runtime,
  ctx: &Context,
  job: &Job,
  run: &str,
  sheet: &Sheet,
  call: &ToolCall,
  offered: &[Offered],
  cancel: &CancellationToken,
  env: &[(String, String)],
  request: &str,
) -> Result<(String, bool)> {
  let args: Value = call.args();
  let name = call.function.name.as_str();
  let idx = sheet.push(Part::Tool { id: call.id.clone(), name: name.into(), args: args.clone(), result: String::new(), status: "running".into() });
  sheet.save(rt).await?;

  let class = tools::class(name, offered);
  let target = tools::target(name, &args);
  let args_text = serde_json::to_string_pretty(&args).unwrap_or_default();
  let action = Action {
    bot: &job.bot,
    chat: &job.chat,
    run,
    interactive: job.origin.interactive(),
    tool: name,
    target: &target,
    args: &args_text,
    class,
    request,
    profile: &ctx.bot.description,
  };
  let set = |result: &str, status: &str| {
    sheet.set(idx, Part::Tool { id: call.id.clone(), name: name.into(), args: args.clone(), result: result.into(), status: status.into() });
  };
  match approve::check(rt, &action).await? {
    Verdict::Run => {}
    Verdict::Deny(why) => {
      set(&why, "denied");
      crate::audit::action(rt, &job.bot, &job.chat, name, &target, "denied", 0).await;
      return Ok((format!("Not allowed: {why}"), false));
    }
    Verdict::Ask(req) => {
      let approval = store::approvals::request(&rt.pool, req).await?;
      sheet.push(Part::Approval { id: approval.id.clone() });
      set("", "waiting");
      sheet.save(rt).await?;
      rt.emit(Event::Approval { id: approval.id.clone() });
      attention(rt, &ctx.bot, &ctx.chat, "Approval needed", &format!("{name} {target}")).await;
      let decision = approve::wait(rt, &approval, cancel).await?;
      resume(rt, &ctx.bot, &ctx.chat).await;
      match decision {
        Decision::Allowed => {}
        Decision::Denied => {
          set("The user denied this.", "denied");
          crate::audit::action(rt, &job.bot, &job.chat, name, &target, "user-denied", 0).await;
          return Ok(("The user denied this action. Do not retry it; continue another way or ask.".into(), false));
        }
        Decision::Expired => {
          set("Approval expired.", "denied");
          crate::audit::action(rt, &job.bot, &job.chat, name, &target, "expired", 0).await;
          return Ok(("The approval expired with no answer. Skip this action and report it.".into(), false));
        }
        Decision::Stopped => {
          set("Stopped.", "denied");
          return Ok(("Stopped.".into(), false));
        }
      }
    }
  }

  let tctx = Ctx { rt, bot: &ctx.bot, chat: &ctx.chat, run, origin: job.origin, cancel, sheet, env };
  let started = std::time::Instant::now();
  let result = tools::call(&tctx, name, &args, offered).await;
  let outcome = if result.is_ok() { "done" } else { "error" };
  crate::audit::action(rt, &job.bot, &job.chat, name, &target, outcome, started.elapsed().as_millis() as u64).await;
  let (text, status, parts, stop) = match result {
    Ok(o) => (o.text, "done", o.parts, o.stop),
    Err(e) => {
      let msg = e.to_string();
      // A plugin that lost its sign-in gets a Connect card in the chat.
      let parts = match offered.iter().find(|o| o.name == name) {
        Some(o) if msg.contains("sign in again") => {
          let pname = store::plugins::get(&rt.pool, &o.plugin).await.map(|p| p.name).unwrap_or_default();
          vec![Part::Connect { plugin: pname, status: "needed".into() }]
        }
        _ => Vec::new(),
      };
      (format!("[error] {msg}"), "error", parts, false)
    }
  };
  let shown = computer::clip::clip(&text, 20_000);
  set(&shown, status);
  for p in parts {
    sheet.push(p);
  }
  Ok((shown, stop))
}

async fn secret_env(rt: &Runtime, bot: &str) -> Vec<(String, String)> {
  let mut out = Vec::new();
  for s in store::secrets::list(&rt.pool, bot).await.unwrap_or_default() {
    if let Some(v) = config::secret::get(&s.key()) {
      out.push((s.name.clone(), v));
    }
  }
  out
}

pub async fn set_status(rt: &Runtime, bot: &str, status: &str) {
  let _ = bots::set_status(&rt.pool, bot, status).await;
  rt.emit(Event::BotStatus { bot: bot.to_string(), status: status.to_string() });
}

/// The Bot is blocked on the user.
pub async fn attention(rt: &Runtime, bot: &store::Bot, chat: &store::Chat, title: &str, body: &str) {
  set_status(rt, &bot.id, bots::WAITING).await;
  let _ = chats::set_attention(&rt.pool, &chat.id, true).await;
  rt.emit(Event::ChatsChanged);
  notify(rt, bot, Some(&chat.id), &format!("{}: {title}", bot.name), body).await;
}

pub async fn resume(rt: &Runtime, bot: &store::Bot, chat: &store::Chat) {
  set_status(rt, &bot.id, bots::WORKING).await;
  let _ = chats::set_attention(&rt.pool, &chat.id, false).await;
  rt.emit(Event::ChatsChanged);
}

/// Post an in-app notification (and let the UI raise an OS one). Hidden
/// Bots and Bots with notifications off stay quiet.
pub async fn notify(rt: &Runtime, bot: &store::Bot, chat: Option<&str>, title: &str, body: &str) {
  let _ = store::notifications::add(&rt.pool, Some(&bot.id), chat, "info", title, body).await;
  if bot.hidden || !bot.notifications || !rt.settings().notifications {
    return;
  }
  rt.emit(Event::Notify { bot: Some(bot.id.clone()), chat: chat.map(str::to_string), title: title.to_string(), body: body.to_string() });
}

/// A generated group name: the fast model's pick, or the members' names.
pub async fn group_name(rt: &Runtime, ids: &[String]) -> String {
  let mut names = Vec::new();
  for id in ids {
    if let Ok(b) = bots::get(&rt.pool, id).await {
      names.push(b.name);
    }
  }
  let fallback = names.join(", ");
  let Ok(fast) = rt.fast().await else { return fallback };
  let ask = vec![Msg::user(format!("Name a group chat for these AI teammates in 2 to 4 words, no quotes or punctuation: {fallback}"))];
  match fast.complete(ask, Some(20)).await {
    Ok(t) if !t.trim().is_empty() && t.len() < 60 => t.trim().trim_matches('"').to_string(),
    _ => fallback,
  }
}

/// Stop a Bot now: cancel its work and drop its queue. Completed actions
/// are not undone.
pub fn stop(rt: &Runtime, bot: &str) {
  rt.queues.stop(bot);
}

/// With `ASYLUM_TRACE=<file>`, append each model exchange to that file.
fn trace(line: &str) {
  if let Ok(path) = std::env::var("ASYLUM_TRACE") {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
      let _ = writeln!(f, "{line}");
    }
  }
}

pub fn is_stop(text: &str) -> bool {
  let t = text.trim().trim_end_matches(['.', '!']).to_lowercase();
  t == "stop now" || t == "stop"
}
