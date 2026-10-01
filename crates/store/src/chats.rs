use crate::{newid, now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub const DIRECT: &str = "direct";
pub const GROUP: &str = "group";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Chat {
  pub id: String,
  pub kind: String,
  pub title: String,
  pub description: String,
  pub bot_id: Option<String>,
  pub section_id: Option<String>,
  pub pinned: bool,
  pub hidden: bool,
  pub unread: bool,
  pub attention: bool,
  pub draft: String,
  pub position: i64,
  pub created: i64,
  pub updated: i64,
}

impl Chat {
  pub fn is_group(&self) -> bool {
    self.kind == GROUP
  }
}

async fn insert(pool: &Pool, kind: &str, title: &str, bot: Option<&str>) -> Result<Chat> {
  let id = newid();
  let t = now();
  sqlx::query("INSERT INTO chats (id, kind, title, bot_id, created, updated) VALUES (?, ?, ?, ?, ?, ?)")
    .bind(&id)
    .bind(kind)
    .bind(title)
    .bind(bot)
    .bind(t)
    .bind(t)
    .execute(pool)
    .await?;
  get(pool, &id).await
}

/// A new conversation with one Bot.
pub async fn create_direct(pool: &Pool, bot: &str, title: &str) -> Result<Chat> {
  let chat = insert(pool, DIRECT, title, Some(bot)).await?;
  add_member(pool, &chat.id, bot).await?;
  Ok(chat)
}

/// The Bot's most recent conversation, created if it has none.
pub async fn direct(pool: &Pool, bot: &str) -> Result<Chat> {
  let found: Option<Chat> = sqlx::query_as(
    "SELECT * FROM chats WHERE kind = 'direct' AND bot_id = ? ORDER BY updated DESC LIMIT 1",
  )
  .bind(bot)
  .fetch_optional(pool)
  .await?;
  match found {
    Some(c) => Ok(c),
    None => create_direct(pool, bot, "").await,
  }
}

pub async fn for_bot(pool: &Pool, bot: &str) -> Result<Vec<Chat>> {
  Ok(
    sqlx::query_as("SELECT * FROM chats WHERE kind = 'direct' AND bot_id = ? ORDER BY updated DESC")
      .bind(bot)
      .fetch_all(pool)
      .await?,
  )
}

pub async fn create_group(pool: &Pool, title: &str, bots: &[String]) -> Result<Chat> {
  let chat = insert(pool, GROUP, title, None).await?;
  for b in bots {
    add_member(pool, &chat.id, b).await?;
  }
  Ok(chat)
}

pub async fn groups(pool: &Pool) -> Result<Vec<Chat>> {
  Ok(
    sqlx::query_as("SELECT * FROM chats WHERE kind = 'group' ORDER BY pinned DESC, updated DESC")
      .fetch_all(pool)
      .await?,
  )
}

pub async fn get(pool: &Pool, id: &str) -> Result<Chat> {
  Ok(
    sqlx::query_as("SELECT * FROM chats WHERE id = ?")
      .bind(id)
      .fetch_one(pool)
      .await?,
  )
}

pub async fn members(pool: &Pool, chat: &str) -> Result<Vec<String>> {
  Ok(
    sqlx::query_scalar(
      "SELECT m.bot_id FROM chat_members m JOIN bots b ON b.id = m.bot_id WHERE m.chat_id = ? ORDER BY b.position",
    )
    .bind(chat)
    .fetch_all(pool)
    .await?,
  )
}

pub async fn add_member(pool: &Pool, chat: &str, bot: &str) -> Result<()> {
  sqlx::query("INSERT OR IGNORE INTO chat_members (chat_id, bot_id) VALUES (?, ?)")
    .bind(chat)
    .bind(bot)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn remove_member(pool: &Pool, chat: &str, bot: &str) -> Result<()> {
  sqlx::query("DELETE FROM chat_members WHERE chat_id = ? AND bot_id = ?")
    .bind(chat)
    .bind(bot)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn rename(pool: &Pool, id: &str, title: &str) -> Result<()> {
  sqlx::query("UPDATE chats SET title = ? WHERE id = ?")
    .bind(title)
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn pin(pool: &Pool, id: &str, on: bool) -> Result<()> {
  sqlx::query("UPDATE chats SET pinned = ? WHERE id = ?")
    .bind(on)
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

async fn set_col<T>(pool: &Pool, id: &str, column: &str, value: T) -> Result<()>
where
  T: for<'q> sqlx::Encode<'q, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite> + Send + 'static,
{
  sqlx::query(&format!("UPDATE chats SET {column} = ? WHERE id = ?"))
    .bind(value)
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn set_unread(pool: &Pool, id: &str, on: bool) -> Result<()> {
  set_col(pool, id, "unread", on).await
}

pub async fn set_attention(pool: &Pool, id: &str, on: bool) -> Result<()> {
  set_col(pool, id, "attention", on).await
}

pub async fn hide(pool: &Pool, id: &str, on: bool) -> Result<()> {
  set_col(pool, id, "hidden", on).await
}

pub async fn save_draft(pool: &Pool, id: &str, draft: &str) -> Result<()> {
  set_col(pool, id, "draft", draft.to_string()).await
}

pub async fn set_description(pool: &Pool, id: &str, text: &str) -> Result<()> {
  set_col(pool, id, "description", text.to_string()).await
}

pub async fn set_section(pool: &Pool, id: &str, section: Option<&str>) -> Result<()> {
  set_col(pool, id, "section_id", section.map(str::to_string)).await
}

/// Every chat a Bot takes part in: its own conversations and its groups.
pub async fn involving(pool: &Pool, bot: &str) -> Result<Vec<Chat>> {
  Ok(
    sqlx::query_as(
      "SELECT c.* FROM chats c JOIN chat_members m ON m.chat_id = c.id WHERE m.bot_id = ? ORDER BY c.updated DESC",
    )
    .bind(bot)
    .fetch_all(pool)
    .await?,
  )
}

pub async fn all(pool: &Pool) -> Result<Vec<Chat>> {
  Ok(sqlx::query_as("SELECT * FROM chats ORDER BY updated DESC").fetch_all(pool).await?)
}

pub async fn touch(pool: &Pool, id: &str) -> Result<()> {
  sqlx::query("UPDATE chats SET updated = ? WHERE id = ?")
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn delete(pool: &Pool, id: &str) -> Result<()> {
  sqlx::query("DELETE FROM chats WHERE id = ?")
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

#[cfg(test)]
#[path = "../tests/chats.rs"]
mod tests;
