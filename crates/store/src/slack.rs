use crate::{now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct App {
  pub bot_id: String,
  pub team: String,
  pub bot_user: String,
  pub status: String,
  pub created: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Link {
  pub slack_user: String,
  pub name: String,
  pub created: i64,
}

pub async fn save_app(pool: &Pool, bot: &str, team: &str, bot_user: &str) -> Result<()> {
  sqlx::query("INSERT OR REPLACE INTO slack_apps (bot_id, team, bot_user, status, created) VALUES (?, ?, ?, 'connected', ?)")
    .bind(bot)
    .bind(team)
    .bind(bot_user)
    .bind(now())
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn app(pool: &Pool, bot: &str) -> Result<Option<App>> {
  Ok(sqlx::query_as("SELECT * FROM slack_apps WHERE bot_id = ?").bind(bot).fetch_optional(pool).await?)
}

pub async fn apps(pool: &Pool) -> Result<Vec<App>> {
  Ok(sqlx::query_as("SELECT * FROM slack_apps").fetch_all(pool).await?)
}

pub async fn set_status(pool: &Pool, bot: &str, status: &str) -> Result<()> {
  sqlx::query("UPDATE slack_apps SET status = ? WHERE bot_id = ?").bind(status).bind(bot).execute(pool).await?;
  Ok(())
}

pub async fn remove_app(pool: &Pool, bot: &str) -> Result<()> {
  sqlx::query("DELETE FROM slack_apps WHERE bot_id = ?").bind(bot).execute(pool).await?;
  sqlx::query("DELETE FROM slack_chats WHERE bot_id = ?").bind(bot).execute(pool).await?;
  Ok(())
}

pub async fn links(pool: &Pool) -> Result<Vec<Link>> {
  Ok(sqlx::query_as("SELECT * FROM slack_links ORDER BY name").fetch_all(pool).await?)
}

pub async fn link(pool: &Pool, user: &str, name: &str) -> Result<()> {
  sqlx::query("INSERT OR REPLACE INTO slack_links (slack_user, name, created) VALUES (?, ?, ?)")
    .bind(user.trim())
    .bind(name)
    .bind(now())
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn unlink(pool: &Pool, user: &str) -> Result<()> {
  sqlx::query("DELETE FROM slack_links WHERE slack_user = ?").bind(user).execute(pool).await?;
  Ok(())
}

/// The chat a Slack conversation maps to, if any.
pub async fn chat_for(pool: &Pool, bot: &str, key: &str) -> Result<Option<String>> {
  Ok(sqlx::query_scalar("SELECT chat_id FROM slack_chats WHERE bot_id = ? AND key = ?").bind(bot).bind(key).fetch_optional(pool).await?)
}

pub async fn map_chat(pool: &Pool, bot: &str, key: &str, chat: &str, follow: bool) -> Result<()> {
  sqlx::query("INSERT INTO slack_chats (bot_id, key, chat_id, follow) VALUES (?, ?, ?, ?) ON CONFLICT(bot_id, key) DO UPDATE SET follow = MAX(follow, excluded.follow)")
    .bind(bot)
    .bind(key)
    .bind(chat)
    .bind(follow)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn followed(pool: &Pool, bot: &str) -> Result<Vec<String>> {
  Ok(sqlx::query_scalar("SELECT key FROM slack_chats WHERE bot_id = ? AND follow = 1").bind(bot).fetch_all(pool).await?)
}

/// The workspace needs an admin to approve the app before it can be installed.
pub async fn await_approval(pool: &Pool, bot: &str) -> Result<()> {
  sqlx::query("INSERT OR REPLACE INTO slack_apps (bot_id, team, bot_user, status, created) VALUES (?, '', '', 'awaiting', ?)")
    .bind(bot)
    .bind(now())
    .execute(pool)
    .await?;
  Ok(())
}
