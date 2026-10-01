//! Shareable Bot templates: identity, description, skills, and routines —
//! never the computer, sign-ins, memory, or history.

use crate::{newid, now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub const PUBLIC: &str = "public";
pub const TEAM: &str = "team";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Template {
  pub id: String,
  pub bot_id: Option<String>,
  pub name: String,
  pub visibility: String,
  pub payload: String,
  pub created: i64,
  pub updated: i64,
}

pub async fn upsert(pool: &Pool, bot: &str, name: &str, visibility: &str, payload: &str) -> Result<Template> {
  let t = now();
  let existing: Option<Template> = sqlx::query_as("SELECT * FROM templates WHERE bot_id = ?")
    .bind(bot)
    .fetch_optional(pool)
    .await?;
  let id = match existing {
    Some(e) => {
      sqlx::query("UPDATE templates SET name = ?, visibility = ?, payload = ?, updated = ? WHERE id = ?")
        .bind(name)
        .bind(visibility)
        .bind(payload)
        .bind(t)
        .bind(&e.id)
        .execute(pool)
        .await?;
      e.id
    }
    None => {
      let id = newid();
      sqlx::query("INSERT INTO templates (id, bot_id, name, visibility, payload, created, updated) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(bot)
        .bind(name)
        .bind(visibility)
        .bind(payload)
        .bind(t)
        .bind(t)
        .execute(pool)
        .await?;
      id
    }
  };
  get(pool, &id).await
}

pub async fn get(pool: &Pool, id: &str) -> Result<Template> {
  Ok(sqlx::query_as("SELECT * FROM templates WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn for_bot(pool: &Pool, bot: &str) -> Result<Option<Template>> {
  Ok(sqlx::query_as("SELECT * FROM templates WHERE bot_id = ?").bind(bot).fetch_optional(pool).await?)
}

pub async fn set_visibility(pool: &Pool, id: &str, visibility: &str) -> Result<()> {
  sqlx::query("UPDATE templates SET visibility = ? WHERE id = ?").bind(visibility).bind(id).execute(pool).await?;
  Ok(())
}
