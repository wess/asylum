use crate::event::Event;
use crate::part::Part;
use crate::queue::{Job, Origin};
use crate::route::{self, Target};
use crate::runtime::Runtime;
use anyhow::{bail, Result};
use grok::Message as Msg;
use std::path::PathBuf;
use store::{bots, chats, messages, Message};

/// Send the user's message. "Stop now" halts the Bots in this chat; any
/// other message preempts their current work in this chat and starts a
/// reply. In a group, mentions pick who answers; otherwise the Bots decide.
pub async fn send(rt: &Runtime, chat: &str, text: &str, files: &[PathBuf], thread: Option<&str>) -> Result<Message> {
  let pool = &rt.pool;
  let c = chats::get(pool, chat).await?;
  if text.trim().is_empty() && files.is_empty() {
    bail!("nothing to send");
  }
  let parts = crate::api::attach::ingest(&rt.computer.workspace(), files)?;
  let m = messages::add(pool, messages::New {
    chat,
    bot: None,
    role: messages::USER,
    body: text,
    parts: &crate::part::to_values(&parts),
    status: messages::DONE,
    run: None,
    thread,
  })
  .await?;
  chats::save_draft(pool, chat, "").await?;
  chats::set_attention(pool, chat, false).await?;
  rt.emit(Event::Message { chat: chat.into(), message: m.id.clone() });
  crate::ticker::seen(rt).await;

  let members = chats::members(pool, chat).await?;
  if crate::turn::is_stop(text) {
    for b in &members {
      crate::turn::stop(rt, b);
    }
    return Ok(m);
  }
  let responders = if c.is_group() { pick(rt, &c, &members, text).await? } else { members };
  for b in responders {
    crate::turn::enqueue(rt, Job { bot: b, chat: chat.into(), origin: Origin::Chat, routine: None, note: String::new() });
  }
  Ok(m)
}

async fn pick(rt: &Runtime, chat: &store::Chat, members: &[String], text: &str) -> Result<Vec<String>> {
  let mut pairs = Vec::new();
  for id in members {
    let b = bots::get(&rt.pool, id).await?;
    pairs.push((b.id, b.name));
  }
  Ok(match route::targets(text, &pairs) {
    Target::Everyone => members.to_vec(),
    Target::Named(ids) => ids,
    Target::Undecided => decide(rt, chat, &pairs, text).await,
  })
}

/// Let the Bots decide: the fast model reads the roster and the message
/// and names who should answer (at least one).
async fn decide(rt: &Runtime, chat: &store::Chat, pairs: &[(String, String)], text: &str) -> Vec<String> {
  let first = pairs.first().map(|(id, _)| vec![id.clone()]).unwrap_or_default();
  let Ok(fast) = rt.fast().await else { return first };
  let mut roster = String::new();
  for (id, name) in pairs {
    let label = bots::get(&rt.pool, id).await.map(|b| b.label).unwrap_or_default();
    roster.push_str(&format!("- {name}: {label}\n"));
  }
  let prompt = format!(
    "A user wrote in a group chat{}. Members:\n{roster}\nMessage: {text}\n\nWhich member(s) should respond? Usually one. \
Reply with only their names separated by commas.",
    if chat.description.is_empty() { String::new() } else { format!(" (about: {})", chat.description) }
  );
  match fast.complete(vec![Msg::user(prompt)], Some(40)).await {
    Ok(ans) => {
      let ids = route::parse_pick(&ans, pairs);
      if ids.is_empty() {
        first
      } else {
        ids
      }
    }
    Err(_) => first,
  }
}

/// Stop every Bot working in a chat.
pub async fn stop(rt: &Runtime, chat: &str) -> Result<()> {
  for b in chats::members(&rt.pool, chat).await? {
    crate::turn::stop(rt, &b);
  }
  Ok(())
}

/// Ask the chat's Bot(s) to try the last message again.
pub async fn retry(rt: &Runtime, chat: &str) -> Result<()> {
  let c = chats::get(&rt.pool, chat).await?;
  let last_user = messages::recent(&rt.pool, chat, 30).await?.into_iter().rev().find(|m| m.role == messages::USER);
  let members = chats::members(&rt.pool, chat).await?;
  let who = match (&last_user, c.is_group()) {
    (Some(m), true) => pick(rt, &c, &members, &m.body).await?,
    _ => members,
  };
  for b in who {
    crate::turn::enqueue(rt, Job { bot: b, chat: chat.into(), origin: Origin::Chat, routine: None, note: String::new() });
  }
  Ok(())
}

/// Edit a sent message: drop it and everything after, then resend.
pub async fn edit(rt: &Runtime, message: &str, text: &str) -> Result<Message> {
  let m = messages::get(&rt.pool, message).await?;
  stop(rt, &m.chat_id).await?;
  messages::truncate_from(&rt.pool, &m.chat_id, message).await?;
  let files: Vec<PathBuf> = crate::part::parse(&m.parts)
    .into_iter()
    .filter_map(|p| match p {
      Part::Attachment { path, .. } => Some(PathBuf::from(path)),
      _ => None,
    })
    .collect();
  rt.emit(Event::ChatsChanged);
  send(rt, &m.chat_id, text, &files, m.thread_id.as_deref()).await
}

pub async fn open(rt: &Runtime, chat: &str) -> Result<()> {
  chats::set_unread(&rt.pool, chat, false).await?;
  let c = chats::get(&rt.pool, chat).await?;
  if let Some(b) = &c.bot_id {
    store::notifications::mark_bot_read(&rt.pool, b).await?;
    let bot = bots::get(&rt.pool, b).await?;
    if bot.status == bots::DONE {
      crate::turn::set_status(rt, b, bots::IDLE).await;
    }
  }
  crate::ticker::seen(rt).await;
  rt.emit(Event::ChatsChanged);
  Ok(())
}
