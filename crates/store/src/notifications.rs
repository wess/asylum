use crate::{newid, now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Notification {
  pub id: String,
  pub bot_id: Option<String>,
  pub chat_id: Option<String>,
  pub kind: String,
  pub title: String,
  pub body: String,
  pub read: bool,
  pub created: i64,
}

pub async fn add(pool: &Pool, bot: Option<&str>, chat: Option<&str>, kind: &str, title: &str, body: &str) -> Result<Notification> {
  let id = newid();
  sqlx::query("INSERT INTO notifications (id, bot_id, chat_id, kind, title, body, created) VALUES (?, ?, ?, ?, ?, ?, ?)")
    .bind(&id)
    .bind(bot)
    .bind(chat)
    .bind(kind)
    .bind(title)
    .bind(body)
    .bind(now())
    .execute(pool)
    .await?;
  Ok(sqlx::query_as("SELECT * FROM notifications WHERE id = ?").bind(&id).fetch_one(pool).await?)
}

pub async fn list(pool: &Pool, limit: i64) -> Result<Vec<Notification>> {
  Ok(sqlx::query_as("SELECT * FROM notifications ORDER BY created DESC LIMIT ?").bind(limit).fetch_all(pool).await?)
}

pub async fn unread(pool: &Pool) -> Result<i64> {
  Ok(sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE read = 0").fetch_one(pool).await?)
}

/// Unread counts per Bot, for sidebar badges.
pub async fn unread_by_bot(pool: &Pool) -> Result<Vec<(String, i64)>> {
  Ok(
    sqlx::query_as("SELECT bot_id, COUNT(*) FROM notifications WHERE read = 0 AND bot_id IS NOT NULL GROUP BY bot_id")
      .fetch_all(pool)
      .await?,
  )
}

pub async fn mark_read(pool: &Pool, id: &str) -> Result<()> {
  sqlx::query("UPDATE notifications SET read = 1 WHERE id = ?").bind(id).execute(pool).await?;
  Ok(())
}

pub async fn mark_bot_read(pool: &Pool, bot: &str) -> Result<()> {
  sqlx::query("UPDATE notifications SET read = 1 WHERE bot_id = ?").bind(bot).execute(pool).await?;
  Ok(())
}

pub async fn mark_all_read(pool: &Pool) -> Result<()> {
  sqlx::query("UPDATE notifications SET read = 1").execute(pool).await?;
  Ok(())
}

pub async fn clear(pool: &Pool) -> Result<()> {
  sqlx::query("DELETE FROM notifications").execute(pool).await?;
  Ok(())
}
