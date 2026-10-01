//! Routines: a workflow assigned to one Bot, run on a schedule or when an
//! event arrives. Run history lives in `runs` (see `runs::for_routine`).

use crate::{newid, now, Pool};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const LIMIT_PER_BOT: i64 = 50;

pub const SCHEDULE: &str = "schedule";
pub const INTERVAL: &str = "interval";
pub const WEBHOOK: &str = "webhook";
pub const TRIGGERS: [&str; 9] = [
  SCHEDULE, INTERVAL, WEBHOOK, "slack", "github", "linear", "sentry", "pagerduty", "email",
];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Routine {
  pub id: String,
  pub bot_id: String,
  pub name: String,
  pub instruction: String,
  pub trigger: String,
  pub schedule: String,
  pub filter: String,
  pub webhook_key: String,
  pub active: bool,
  pub last_run: Option<i64>,
  pub next_run: Option<i64>,
  pub created: i64,
  pub updated: i64,
}

impl Routine {
  pub fn filter(&self) -> serde_json::Value {
    serde_json::from_str(&self.filter).unwrap_or_default()
  }

  /// Everything except timed triggers arrives over the webhook endpoint.
  pub fn is_event(&self) -> bool {
    self.trigger != SCHEDULE && self.trigger != INTERVAL
  }
}

#[derive(Clone, Debug, Default)]
pub struct New {
  pub bot: String,
  pub name: String,
  pub instruction: String,
  pub trigger: String,
  pub schedule: String,
  pub filter: serde_json::Value,
  pub next_run: Option<i64>,
}

pub async fn create(pool: &Pool, r: New) -> Result<Routine> {
  let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM routines WHERE bot_id = ?")
    .bind(&r.bot)
    .fetch_one(pool)
    .await?;
  if count >= LIMIT_PER_BOT {
    bail!("this Bot already has {LIMIT_PER_BOT} routines");
  }
  if !TRIGGERS.contains(&r.trigger.as_str()) {
    bail!("unknown trigger {}", r.trigger);
  }
  let id = newid();
  let t = now();
  let key = format!("{}{}", newid(), newid());
  sqlx::query(
    "INSERT INTO routines (id, bot_id, name, instruction, trigger, schedule, filter, webhook_key, next_run, created, updated)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(&id)
  .bind(&r.bot)
  .bind(&r.name)
  .bind(&r.instruction)
  .bind(&r.trigger)
  .bind(&r.schedule)
  .bind(r.filter.to_string())
  .bind(key)
  .bind(r.next_run)
  .bind(t)
  .bind(t)
  .execute(pool)
  .await?;
  get(pool, &id).await
}

pub async fn get(pool: &Pool, id: &str) -> Result<Routine> {
  Ok(sqlx::query_as("SELECT * FROM routines WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn find(pool: &Pool, bot: &str, name: &str) -> Result<Option<Routine>> {
  Ok(
    sqlx::query_as("SELECT * FROM routines WHERE bot_id = ? AND lower(name) = lower(?)")
      .bind(bot)
      .bind(name.trim())
      .fetch_optional(pool)
      .await?,
  )
}

pub async fn for_bot(pool: &Pool, bot: &str) -> Result<Vec<Routine>> {
  Ok(
    sqlx::query_as("SELECT * FROM routines WHERE bot_id = ? ORDER BY created")
      .bind(bot)
      .fetch_all(pool)
      .await?,
  )
}

pub async fn all(pool: &Pool) -> Result<Vec<Routine>> {
  Ok(sqlx::query_as("SELECT * FROM routines ORDER BY next_run IS NULL, next_run").fetch_all(pool).await?)
}

/// Active timed routines whose next run is at or before `at`.
pub async fn due(pool: &Pool, at: i64) -> Result<Vec<Routine>> {
  Ok(
    sqlx::query_as("SELECT * FROM routines WHERE active = 1 AND next_run IS NOT NULL AND next_run <= ? ORDER BY next_run")
      .bind(at)
      .fetch_all(pool)
      .await?,
  )
}

pub async fn by_key(pool: &Pool, id: &str, key: &str) -> Result<Option<Routine>> {
  Ok(
    sqlx::query_as("SELECT * FROM routines WHERE id = ? AND webhook_key = ?")
      .bind(id)
      .bind(key)
      .fetch_optional(pool)
      .await?,
  )
}

pub async fn update(pool: &Pool, id: &str, r: &New) -> Result<()> {
  sqlx::query(
    "UPDATE routines SET name = ?, instruction = ?, trigger = ?, schedule = ?, filter = ?, next_run = ?, updated = ? WHERE id = ?",
  )
  .bind(&r.name)
  .bind(&r.instruction)
  .bind(&r.trigger)
  .bind(&r.schedule)
  .bind(r.filter.to_string())
  .bind(r.next_run)
  .bind(now())
  .bind(id)
  .execute(pool)
  .await?;
  Ok(())
}

pub async fn set_active(pool: &Pool, id: &str, on: bool, next_run: Option<i64>) -> Result<()> {
  sqlx::query("UPDATE routines SET active = ?, next_run = ?, updated = ? WHERE id = ?")
    .bind(on)
    .bind(next_run)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn pause_all(pool: &Pool) -> Result<u64> {
  Ok(sqlx::query("UPDATE routines SET active = 0").execute(pool).await?.rows_affected())
}

pub async fn ran(pool: &Pool, id: &str, at: i64, next_run: Option<i64>) -> Result<()> {
  sqlx::query("UPDATE routines SET last_run = ?, next_run = ? WHERE id = ?")
    .bind(at)
    .bind(next_run)
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

/// Moving a routine between Bots (Publish-to-Team "move").
pub async fn reassign(pool: &Pool, id: &str, bot: &str) -> Result<()> {
  sqlx::query("UPDATE routines SET bot_id = ? WHERE id = ?").bind(bot).bind(id).execute(pool).await?;
  Ok(())
}

pub async fn delete(pool: &Pool, id: &str) -> Result<()> {
  sqlx::query("DELETE FROM routines WHERE id = ?").bind(id).execute(pool).await?;
  Ok(())
}

#[cfg(test)]
#[path = "../tests/routines.rs"]
mod tests;
