use crate::{newid, now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub const PENDING: &str = "pending";
pub const APPROVED: &str = "approved";
pub const DENIED: &str = "denied";
pub const EXPIRED: &str = "expired";
pub const CANCELED: &str = "canceled";

/// A plain approval, one raised by Auto-review ("Review an action"), a
/// local-computer command, or a Team Bot using a personal connector.
pub const APPROVAL: &str = "approval";
pub const REVIEW: &str = "review";
pub const LOCAL: &str = "local";
pub const CONNECTOR: &str = "connector";

/// Approvals raised by work that started without the user expire.
pub const EXPIRY_MS: i64 = 10 * 60 * 1000;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Approval {
  pub id: String,
  pub bot_id: String,
  pub chat_id: Option<String>,
  pub run_id: Option<String>,
  pub message_id: Option<String>,
  pub kind: String,
  pub tool: String,
  pub target: String,
  pub args: String,
  pub reason: String,
  pub interactive: bool,
  pub status: String,
  pub created: i64,
  pub decided: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct New {
  pub bot: String,
  pub chat: Option<String>,
  pub run: Option<String>,
  pub kind: String,
  pub tool: String,
  pub target: String,
  pub args: String,
  pub reason: String,
  pub interactive: bool,
}

pub async fn request(pool: &Pool, a: New) -> Result<Approval> {
  let id = newid();
  sqlx::query(
    "INSERT INTO approvals (id, bot_id, chat_id, run_id, kind, tool, target, args, reason, interactive, created)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(&id)
  .bind(&a.bot)
  .bind(&a.chat)
  .bind(&a.run)
  .bind(if a.kind.is_empty() { APPROVAL } else { a.kind.as_str() })
  .bind(&a.tool)
  .bind(&a.target)
  .bind(&a.args)
  .bind(&a.reason)
  .bind(a.interactive)
  .bind(now())
  .execute(pool)
  .await?;
  get(pool, &id).await
}

pub async fn attach(pool: &Pool, id: &str, message: &str) -> Result<()> {
  sqlx::query("UPDATE approvals SET message_id = ? WHERE id = ?").bind(message).bind(id).execute(pool).await?;
  Ok(())
}

pub async fn get(pool: &Pool, id: &str) -> Result<Approval> {
  Ok(sqlx::query_as("SELECT * FROM approvals WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn pending(pool: &Pool) -> Result<Vec<Approval>> {
  Ok(sqlx::query_as("SELECT * FROM approvals WHERE status = 'pending' ORDER BY created").fetch_all(pool).await?)
}

pub async fn decide(pool: &Pool, id: &str, status: &str) -> Result<bool> {
  Ok(
    sqlx::query("UPDATE approvals SET status = ?, decided = ? WHERE id = ? AND status = 'pending'")
      .bind(status)
      .bind(now())
      .bind(id)
      .execute(pool)
      .await?
      .rows_affected()
      > 0,
  )
}

/// Expire non-interactive approvals older than the window. Returns them.
pub async fn expire_stale(pool: &Pool, at: i64) -> Result<Vec<Approval>> {
  let stale: Vec<Approval> = sqlx::query_as(
    "SELECT * FROM approvals WHERE status = 'pending' AND interactive = 0 AND created <= ?",
  )
  .bind(at - EXPIRY_MS)
  .fetch_all(pool)
  .await?;
  for a in &stale {
    decide(pool, &a.id, EXPIRED).await?;
  }
  Ok(stale)
}

/// Anything still pending when the app last quit can no longer resume.
pub async fn cancel_all(pool: &Pool) -> Result<u64> {
  Ok(
    sqlx::query("UPDATE approvals SET status = 'canceled', decided = ? WHERE status = 'pending'")
      .bind(now())
      .execute(pool)
      .await?
      .rows_affected(),
  )
}
