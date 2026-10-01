use crate::{newid, now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub const RUNNING: &str = "running";
pub const WAITING: &str = "waiting";
pub const DONE: &str = "done";
pub const FAILED: &str = "failed";
pub const STOPPED: &str = "stopped";

/// Where work came from. Anything but `chat` started without the user, so
/// its approvals expire.
pub const CHAT: &str = "chat";
pub const ROUTINE: &str = "routine";
pub const TEST: &str = "test";
pub const HANDOFF: &str = "handoff";

/// Run history kept per routine.
pub const HISTORY: i64 = 20;

/// One unit of Bot work: a reply to a message, a routine firing, or a
/// handoff from another Bot.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Run {
  pub id: String,
  pub bot_id: String,
  pub chat_id: Option<String>,
  pub routine_id: Option<String>,
  pub origin: String,
  pub status: String,
  pub summary: String,
  pub error: String,
  pub steps: i64,
  pub tokens: i64,
  pub started: i64,
  pub finished: Option<i64>,
}

pub async fn start(pool: &Pool, bot: &str, chat: Option<&str>, routine: Option<&str>, origin: &str) -> Result<Run> {
  let id = newid();
  sqlx::query("INSERT INTO runs (id, bot_id, chat_id, routine_id, origin, started) VALUES (?, ?, ?, ?, ?, ?)")
    .bind(&id)
    .bind(bot)
    .bind(chat)
    .bind(routine)
    .bind(origin)
    .bind(now())
    .execute(pool)
    .await?;
  get(pool, &id).await
}

pub async fn get(pool: &Pool, id: &str) -> Result<Run> {
  Ok(sqlx::query_as("SELECT * FROM runs WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn progress(pool: &Pool, id: &str, steps: i64, tokens: i64, status: &str) -> Result<()> {
  sqlx::query("UPDATE runs SET steps = ?, tokens = ?, status = ? WHERE id = ?")
    .bind(steps)
    .bind(tokens)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn finish(pool: &Pool, id: &str, status: &str, summary: &str, error: &str) -> Result<()> {
  sqlx::query("UPDATE runs SET status = ?, summary = ?, error = ?, finished = ? WHERE id = ?")
    .bind(status)
    .bind(summary)
    .bind(error)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
  let routine: Option<String> = sqlx::query_scalar("SELECT routine_id FROM runs WHERE id = ?")
    .bind(id)
    .fetch_one(pool)
    .await?;
  if let Some(r) = routine {
    sqlx::query(
      "DELETE FROM runs WHERE routine_id = ? AND id NOT IN (SELECT id FROM runs WHERE routine_id = ? ORDER BY started DESC LIMIT ?)",
    )
    .bind(&r)
    .bind(&r)
    .bind(HISTORY)
    .execute(pool)
    .await?;
  }
  Ok(())
}

pub async fn for_routine(pool: &Pool, routine: &str) -> Result<Vec<Run>> {
  Ok(
    sqlx::query_as("SELECT * FROM runs WHERE routine_id = ? ORDER BY started DESC LIMIT ?")
      .bind(routine)
      .bind(HISTORY)
      .fetch_all(pool)
      .await?,
  )
}

pub async fn active(pool: &Pool) -> Result<Vec<Run>> {
  Ok(sqlx::query_as("SELECT * FROM runs WHERE status IN ('running', 'waiting') ORDER BY started").fetch_all(pool).await?)
}

pub async fn for_bot(pool: &Pool, bot: &str, limit: i64) -> Result<Vec<Run>> {
  Ok(
    sqlx::query_as("SELECT * FROM runs WHERE bot_id = ? ORDER BY started DESC LIMIT ?")
      .bind(bot)
      .bind(limit)
      .fetch_all(pool)
      .await?,
  )
}

pub async fn recent(pool: &Pool, limit: i64) -> Result<Vec<Run>> {
  Ok(sqlx::query_as("SELECT * FROM runs ORDER BY started DESC LIMIT ?").bind(limit).fetch_all(pool).await?)
}

/// Runs left open by a crash or quit are marked stopped at boot.
pub async fn settle(pool: &Pool) -> Result<u64> {
  Ok(
    sqlx::query("UPDATE runs SET status = 'stopped', finished = ? WHERE status IN ('running', 'waiting')")
      .bind(now())
      .execute(pool)
      .await?
      .rows_affected(),
  )
}
