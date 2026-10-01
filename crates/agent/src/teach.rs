//! Teach a task: the user performs a workflow on the Bot's browser screen
//! (up to ten minutes, no audio); the steps are captured and the Bot drafts
//! a skill from them for the user to review.

use crate::event::Event;
use crate::part::Part;
use crate::runtime::Runtime;
use anyhow::{bail, Result};
use computer::browser::Step;
use grok::Message;
use store::messages;

pub const MAX_MS: i64 = 10 * 60 * 1000;

#[derive(Clone, Debug)]
pub struct Recording {
  pub name: String,
  pub result: String,
  pub started: i64,
  pub chat: String,
}

pub async fn start(rt: &Runtime, bot: &str, chat: &str, name: &str, result: &str) -> Result<()> {
  if rt.recordings.lock().await.contains_key(bot) {
    bail!("already recording");
  }
  rt.browser.start_recording(bot).await?;
  rt.recordings.lock().await.insert(
    bot.to_string(),
    Recording { name: name.to_string(), result: result.to_string(), started: store::now(), chat: chat.to_string() },
  );
  rt.emit(Event::Screen { bot: bot.to_string() });
  Ok(())
}

pub async fn recording(rt: &Runtime, bot: &str) -> Option<Recording> {
  rt.recordings.lock().await.get(bot).cloned()
}

pub fn render(steps: &[Step]) -> String {
  let start = steps.first().map(|s| s.at).unwrap_or(0.0);
  steps
    .iter()
    .enumerate()
    .map(|(i, s)| {
      let secs = ((s.at - start) / 1000.0).max(0.0) as u64;
      let value = if s.value.is_empty() { String::new() } else { format!(" = \"{}\"", s.value) };
      format!("{}. [{}s] {} {}{} ({})", i + 1, secs, s.kind, s.target, value, s.url)
    })
    .collect::<Vec<_>>()
    .join("\n")
}

pub fn prompt(name: &str, result: &str, demo: &str) -> String {
  format!(
    "The user demonstrated a workflow in a browser so you can learn it as a skill.\nSkill name: {name}\nDesired result: {result}\n\n\
Recorded steps (values marked [secret] were hidden):\n{demo}\n\n\
Write the skill as instructions another run of you can follow reliably: when to use it, required inputs and access, \
the steps (generalize specific values into inputs), how to validate success, the output, and which steps need the \
user's approval. Never include secret values.\nReply with only JSON: {{\"description\": \"one line\", \"instructions\": \"...\"}}"
  )
}

/// Stop recording and post a drafted skill card in the chat.
pub async fn stop(rt: &Runtime, bot: &str) -> Result<()> {
  let Some(rec) = rt.recordings.lock().await.remove(bot) else { bail!("not recording") };
  let steps = rt.browser.stop_recording(bot).await.unwrap_or_default();
  rt.emit(Event::Screen { bot: bot.to_string() });
  let demo = render(&steps);
  let (description, instructions) = if steps.is_empty() {
    (rec.result.clone(), "No interaction was captured. Describe the steps in chat to finish this skill.".to_string())
  } else {
    let bot_row = store::bots::get(&rt.pool, bot).await.ok();
    let p = rt.provider(bot_row.as_ref()).await?;
    let text = p.complete(vec![Message::user(prompt(&rec.name, &rec.result, &demo))], None).await?;
    parse(&text).unwrap_or((rec.result.clone(), text))
  };
  let part = Part::SkillDraft { name: rec.name.clone(), description, instructions, status: "draft".into() };
  let body = format!("I drafted a skill from your demo ({} steps). Review and edit it, then save.", steps.len());
  let m = messages::add(&rt.pool, messages::New {
    chat: &rec.chat,
    bot: Some(bot),
    role: messages::BOT,
    body: &body,
    parts: &crate::part::to_values(&[part]),
    status: messages::DONE,
    run: None,
    thread: None,
  })
  .await?;
  rt.emit(Event::Message { chat: rec.chat.clone(), message: m.id });
  Ok(())
}

fn parse(text: &str) -> Option<(String, String)> {
  let s = text.find('{')?;
  let e = text.rfind('}')?;
  let v: serde_json::Value = serde_json::from_str(&text[s..=e]).ok()?;
  Some((v["description"].as_str()?.to_string(), v["instructions"].as_str()?.to_string()))
}

#[cfg(test)]
#[path = "../tests/teach.rs"]
mod tests;
