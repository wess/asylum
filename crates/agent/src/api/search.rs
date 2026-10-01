//! The palette's search: Bots, group chats, messages, files, and routines.

use crate::runtime::Runtime;
use anyhow::Result;
use store::{bots, chats, messages, routines};

#[derive(Clone, Debug, PartialEq)]
pub enum Hit {
  Bot { id: String, name: String, label: String },
  Group { id: String, title: String },
  Message { id: String, chat: String, bot: Option<String>, snippet: String, created: i64 },
  File { path: String, name: String },
  Routine { id: String, bot: String, name: String, when: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
  All,
  Messages,
  Bots,
  Groups,
  Files,
  Routines,
}

pub async fn search(rt: &Runtime, query: &str, scope: Scope) -> Result<Vec<Hit>> {
  let q = query.trim().to_lowercase();
  let mut out = Vec::new();
  let want = |s: Scope| scope == Scope::All || scope == s;
  if want(Scope::Bots) {
    for b in bots::list(&rt.pool).await? {
      if q.is_empty() || b.name.to_lowercase().contains(&q) || b.label.to_lowercase().contains(&q) {
        out.push(Hit::Bot { id: b.id, name: b.name, label: b.label });
      }
    }
  }
  if want(Scope::Groups) {
    for g in chats::groups(&rt.pool).await? {
      if q.is_empty() || g.title.to_lowercase().contains(&q) {
        out.push(Hit::Group { id: g.id, title: g.title });
      }
    }
  }
  if q.is_empty() {
    return Ok(out);
  }
  if want(Scope::Routines) {
    for r in routines::all(&rt.pool).await? {
      if r.name.to_lowercase().contains(&q) || r.instruction.to_lowercase().contains(&q) {
        let when = schedule::describe(&r.trigger, &r.schedule, &r.filter);
        out.push(Hit::Routine { id: r.id, bot: r.bot_id, name: r.name, when });
      }
    }
  }
  if want(Scope::Files) {
    let ws = rt.computer.workspace();
    for p in computer::fs::search(&ws, "~", &q, 30).unwrap_or_default() {
      let path = p.split(':').next().unwrap_or(&p).to_string();
      let name = path.rsplit('/').next().unwrap_or(&path).to_string();
      if !out.iter().any(|h| matches!(h, Hit::File { path: x, .. } if *x == path)) {
        out.push(Hit::File { path, name });
      }
    }
  }
  if want(Scope::Messages) {
    for h in messages::search(&rt.pool, &q, 50).await? {
      out.push(Hit::Message { id: h.id, chat: h.chat_id, bot: h.bot_id, snippet: h.snippet, created: h.created });
    }
  }
  Ok(out)
}
