use crate::{chats, newid, now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const USER: &str = "user";
pub const BOT: &str = "bot";
pub const EVENT: &str = "event";

pub const DONE: &str = "done";
pub const STREAMING: &str = "streaming";
pub const ERROR: &str = "error";
pub const STOPPED: &str = "stopped";

/// One chat entry. `parts` is JSON: attachments, tool calls with their
/// results, reasoning, handoff notes — whatever the renderer shows besides
/// the body.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Message {
  pub id: String,
  pub chat_id: String,
  pub bot_id: Option<String>,
  pub role: String,
  pub body: String,
  pub parts: String,
  pub status: String,
  pub run_id: Option<String>,
  pub thread_id: Option<String>,
  pub reactions: String,
  pub created: i64,
}

impl Message {
  pub fn parts(&self) -> Vec<Value> {
    serde_json::from_str(&self.parts).unwrap_or_default()
  }

  /// Emoji -> count.
  pub fn reactions(&self) -> std::collections::BTreeMap<String, i64> {
    serde_json::from_str(&self.reactions).unwrap_or_default()
  }
}

pub struct New<'a> {
  pub chat: &'a str,
  pub bot: Option<&'a str>,
  pub role: &'a str,
  pub body: &'a str,
  pub parts: &'a [Value],
  pub status: &'a str,
  pub run: Option<&'a str>,
  pub thread: Option<&'a str>,
}

pub async fn add(pool: &Pool, m: New<'_>) -> Result<Message> {
  let id = newid();
  sqlx::query(
    "INSERT INTO messages (id, chat_id, bot_id, role, body, parts, status, run_id, thread_id, created)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(&id)
  .bind(m.chat)
  .bind(m.bot)
  .bind(m.role)
  .bind(m.body)
  .bind(serde_json::to_string(m.parts)?)
  .bind(m.status)
  .bind(m.run)
  .bind(m.thread)
  .bind(now())
  .execute(pool)
  .await?;
  chats::touch(pool, m.chat).await?;
  get(pool, &id).await
}

pub async fn get(pool: &Pool, id: &str) -> Result<Message> {
  Ok(
    sqlx::query_as("SELECT * FROM messages WHERE id = ?")
      .bind(id)
      .fetch_one(pool)
      .await?,
  )
}

/// A chat's main timeline (thread replies excluded).
pub async fn list(pool: &Pool, chat: &str) -> Result<Vec<Message>> {
  Ok(
    sqlx::query_as("SELECT * FROM messages WHERE chat_id = ? AND thread_id IS NULL ORDER BY created, rowid")
      .bind(chat)
      .fetch_all(pool)
      .await?,
  )
}

/// Replies in one thread, oldest first.
pub async fn thread(pool: &Pool, root: &str) -> Result<Vec<Message>> {
  Ok(
    sqlx::query_as("SELECT * FROM messages WHERE thread_id = ? ORDER BY created, rowid")
      .bind(root)
      .fetch_all(pool)
      .await?,
  )
}

/// Reply counts per thread root in a chat.
pub async fn thread_counts(pool: &Pool, chat: &str) -> Result<Vec<(String, i64)>> {
  Ok(
    sqlx::query_as("SELECT thread_id, COUNT(*) FROM messages WHERE chat_id = ? AND thread_id IS NOT NULL GROUP BY thread_id")
      .bind(chat)
      .fetch_all(pool)
      .await?,
  )
}

/// Toggle one reaction on a message.
pub async fn react(pool: &Pool, id: &str, emoji: &str) -> Result<()> {
  let m = get(pool, id).await?;
  let mut r = m.reactions();
  match r.get(emoji) {
    Some(_) => {
      r.remove(emoji);
    }
    None => {
      r.insert(emoji.to_string(), 1);
    }
  }
  sqlx::query("UPDATE messages SET reactions = ? WHERE id = ?")
    .bind(serde_json::to_string(&r)?)
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

/// The newest `n` messages of a chat, oldest first.
pub async fn recent(pool: &Pool, chat: &str, n: i64) -> Result<Vec<Message>> {
  let mut rows: Vec<Message> = sqlx::query_as(
    "SELECT * FROM messages WHERE chat_id = ? ORDER BY created DESC, rowid DESC LIMIT ?",
  )
  .bind(chat)
  .bind(n)
  .fetch_all(pool)
  .await?;
  rows.reverse();
  Ok(rows)
}

pub async fn update(pool: &Pool, id: &str, body: &str, parts: &[Value], status: &str) -> Result<()> {
  sqlx::query("UPDATE messages SET body = ?, parts = ?, status = ? WHERE id = ?")
    .bind(body)
    .bind(serde_json::to_string(parts)?)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn delete(pool: &Pool, id: &str) -> Result<()> {
  sqlx::query("DELETE FROM messages WHERE id = ?")
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

/// Drop `id` and everything after it in its chat — for edit-and-resend.
pub async fn truncate_from(pool: &Pool, chat: &str, id: &str) -> Result<()> {
  let m = get(pool, id).await?;
  sqlx::query("DELETE FROM messages WHERE chat_id = ? AND (created > ? OR (created = ? AND rowid >= (SELECT rowid FROM messages WHERE id = ?)))")
    .bind(chat)
    .bind(m.created)
    .bind(m.created)
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

/// Messages left mid-stream by a crash or quit are marked stopped at boot.
pub async fn settle_streaming(pool: &Pool) -> Result<u64> {
  Ok(
    sqlx::query("UPDATE messages SET status = 'stopped' WHERE status = 'streaming'")
      .execute(pool)
      .await?
      .rows_affected(),
  )
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Hit {
  pub id: String,
  pub chat_id: String,
  pub bot_id: Option<String>,
  pub snippet: String,
  pub created: i64,
}

/// Full-text search over every message body.
pub async fn search(pool: &Pool, query: &str, limit: i64) -> Result<Vec<Hit>> {
  let q = fts_query(query);
  if q.is_empty() {
    return Ok(Vec::new());
  }
  Ok(
    sqlx::query_as(
      "SELECT m.id, m.chat_id, m.bot_id, snippet(messages_fts, 0, '', '', '…', 12) AS snippet, m.created
       FROM messages_fts JOIN messages m ON m.rowid = messages_fts.rowid
       WHERE messages_fts MATCH ? ORDER BY m.created DESC LIMIT ?",
    )
    .bind(q)
    .bind(limit)
    .fetch_all(pool)
    .await?,
  )
}

/// Quote each word as a prefix term so user input can never be FTS syntax.
pub fn fts_query(input: &str) -> String {
  input
    .split_whitespace()
    .map(|w| w.replace('"', ""))
    .filter(|w| !w.is_empty())
    .map(|w| format!("\"{w}\"*"))
    .collect::<Vec<_>>()
    .join(" ")
}

#[cfg(test)]
#[path = "../tests/messages.rs"]
mod tests;

/// (messages, distinct conversations) since a time.
pub async fn activity(pool: &Pool, since: i64) -> Result<(i64, i64)> {
  Ok(sqlx::query_as("SELECT COUNT(*), COUNT(DISTINCT chat_id) FROM messages WHERE created >= ?").bind(since).fetch_one(pool).await?)
}

/// Backdate a message (demo data).
pub async fn set_created(pool: &Pool, id: &str, at: i64) -> Result<()> {
  sqlx::query("UPDATE messages SET created = ? WHERE id = ?").bind(at).bind(id).execute(pool).await?;
  Ok(())
}
