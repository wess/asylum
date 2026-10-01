use crate::{newid, now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Stable working preferences, important facts, and summaries a Bot keeps.
pub const KINDS: [&str; 3] = ["preference", "fact", "summary"];

/// Personal Bot memory, a Team Bot's shared team memory, or a Team Bot's
/// private notes with one person.
pub const BOT: &str = "bot";
pub const TEAM: &str = "team";
pub const PERSON: &str = "person";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Memory {
  pub id: String,
  pub bot_id: String,
  pub kind: String,
  pub scope: String,
  pub person: String,
  pub content: String,
  pub created: i64,
  pub updated: i64,
}

pub async fn add(pool: &Pool, bot: &str, kind: &str, content: &str) -> Result<Memory> {
  add_scoped(pool, bot, kind, BOT, "", content).await
}

pub async fn add_scoped(pool: &Pool, bot: &str, kind: &str, scope: &str, person: &str, content: &str) -> Result<Memory> {
  let id = newid();
  let t = now();
  let kind = if KINDS.contains(&kind) { kind } else { "fact" };
  sqlx::query("INSERT INTO memories (id, bot_id, kind, scope, person, content, created, updated) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
    .bind(&id)
    .bind(bot)
    .bind(kind)
    .bind(scope)
    .bind(person)
    .bind(content.trim())
    .bind(t)
    .bind(t)
    .execute(pool)
    .await?;
  Ok(sqlx::query_as("SELECT * FROM memories WHERE id = ?").bind(&id).fetch_one(pool).await?)
}

pub async fn list(pool: &Pool, bot: &str) -> Result<Vec<Memory>> {
  Ok(
    sqlx::query_as("SELECT * FROM memories WHERE bot_id = ? ORDER BY kind, updated DESC")
      .bind(bot)
      .fetch_all(pool)
      .await?,
  )
}

/// Copy selected memories to another Bot (the Publish-to-Team wizard).
pub async fn copy(pool: &Pool, ids: &[String], to: &str, scope: &str) -> Result<()> {
  for id in ids {
    let m: Memory = sqlx::query_as("SELECT * FROM memories WHERE id = ?").bind(id).fetch_one(pool).await?;
    add_scoped(pool, to, &m.kind, scope, "", &m.content).await?;
  }
  Ok(())
}

pub async fn update(pool: &Pool, id: &str, content: &str) -> Result<()> {
  sqlx::query("UPDATE memories SET content = ?, updated = ? WHERE id = ?")
    .bind(content.trim())
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn delete(pool: &Pool, id: &str) -> Result<()> {
  sqlx::query("DELETE FROM memories WHERE id = ?").bind(id).execute(pool).await?;
  Ok(())
}

pub async fn clear(pool: &Pool, bot: &str) -> Result<()> {
  sqlx::query("DELETE FROM memories WHERE bot_id = ?").bind(bot).execute(pool).await?;
  Ok(())
}

/// Insert unless an entry with the same normalized text already exists.
pub async fn remember(pool: &Pool, bot: &str, kind: &str, content: &str) -> Result<bool> {
  remember_scoped(pool, bot, kind, BOT, "", content).await
}

pub async fn remember_scoped(pool: &Pool, bot: &str, kind: &str, scope: &str, person: &str, content: &str) -> Result<bool> {
  let norm = normalize(content);
  if norm.is_empty() {
    return Ok(false);
  }
  let existing = list(pool, bot).await?;
  if existing
    .iter()
    .any(|m| m.scope == scope && m.person == person && normalize(&m.content) == norm)
  {
    return Ok(false);
  }
  add_scoped(pool, bot, kind, scope, person, content).await?;
  Ok(true)
}

pub fn normalize(s: &str) -> String {
  s.split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
    .trim_end_matches('.')
    .to_lowercase()
}
