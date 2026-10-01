use crate::{newid, now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A user-made sidebar group. Deleting one leaves its Bots and chats
/// unassigned (the foreign keys null out).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Section {
  pub id: String,
  pub name: String,
  pub position: i64,
  pub created: i64,
}

pub async fn create(pool: &Pool, name: &str) -> Result<Section> {
  let id = newid();
  let position: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 1 FROM sections")
    .fetch_one(pool)
    .await?;
  sqlx::query("INSERT INTO sections (id, name, position, created) VALUES (?, ?, ?, ?)")
    .bind(&id)
    .bind(name)
    .bind(position)
    .bind(now())
    .execute(pool)
    .await?;
  Ok(sqlx::query_as("SELECT * FROM sections WHERE id = ?").bind(&id).fetch_one(pool).await?)
}

pub async fn all(pool: &Pool) -> Result<Vec<Section>> {
  Ok(sqlx::query_as("SELECT * FROM sections ORDER BY position").fetch_all(pool).await?)
}

pub async fn rename(pool: &Pool, id: &str, name: &str) -> Result<()> {
  sqlx::query("UPDATE sections SET name = ? WHERE id = ?").bind(name).bind(id).execute(pool).await?;
  Ok(())
}

pub async fn delete(pool: &Pool, id: &str) -> Result<()> {
  sqlx::query("DELETE FROM sections WHERE id = ?").bind(id).execute(pool).await?;
  Ok(())
}

/// Insert a section with a known id (from another device).
pub async fn insert(pool: &Pool, id: &str, name: &str, position: i64) -> Result<()> {
  sqlx::query("INSERT OR IGNORE INTO sections (id, name, position, created) VALUES (?, ?, ?, ?)")
    .bind(id)
    .bind(name)
    .bind(position)
    .bind(now())
    .execute(pool)
    .await?;
  Ok(())
}
