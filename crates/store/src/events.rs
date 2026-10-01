//! Action Recording and the audit log. Actions are metadata only (tool,
//! target, outcome — never arguments or results) and are kept 90 days.

use crate::{newid, now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub const ACTION: &str = "action";
pub const AUDIT: &str = "audit";
pub const RETENTION_MS: i64 = 90 * 24 * 3600 * 1000;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Event {
  pub id: String,
  pub at: i64,
  pub kind: String,
  pub actor: String,
  pub bot_id: Option<String>,
  pub chat_id: Option<String>,
  pub name: String,
  pub target: String,
  pub outcome: String,
  pub detail: String,
}

#[derive(Clone, Debug, Default)]
pub struct New<'a> {
  pub kind: &'a str,
  pub actor: &'a str,
  pub bot: Option<&'a str>,
  pub chat: Option<&'a str>,
  pub name: &'a str,
  pub target: &'a str,
  pub outcome: &'a str,
  pub detail: &'a str,
}

pub async fn add(pool: &Pool, e: New<'_>) -> Result<()> {
  sqlx::query("INSERT INTO events (id, at, kind, actor, bot_id, chat_id, name, target, outcome, detail) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
    .bind(newid())
    .bind(now())
    .bind(e.kind)
    .bind(e.actor)
    .bind(e.bot)
    .bind(e.chat)
    .bind(e.name)
    .bind(e.target.chars().take(500).collect::<String>())
    .bind(e.outcome)
    .bind(e.detail.chars().take(2000).collect::<String>())
    .execute(pool)
    .await?;
  Ok(())
}

/// Newest first, optionally since a time.
pub async fn list(pool: &Pool, kind: &str, since: i64, limit: i64) -> Result<Vec<Event>> {
  Ok(
    sqlx::query_as("SELECT * FROM events WHERE kind = ? AND at >= ? ORDER BY at DESC, rowid DESC LIMIT ?")
      .bind(kind)
      .bind(since)
      .bind(limit)
      .fetch_all(pool)
      .await?,
  )
}

/// Drop actions past retention. Audit entries are kept.
pub async fn prune(pool: &Pool, at: i64) -> Result<u64> {
  Ok(sqlx::query("DELETE FROM events WHERE kind = 'action' AND at < ?").bind(at - RETENTION_MS).execute(pool).await?.rows_affected())
}
