//! One private skill library per account, shared by every Bot, with
//! per-Bot enablement. Packaged skills come from the Marketplace.

use crate::{newid, now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Typed out, taught by demonstration, or saved from a finished task.
pub const WRITTEN: &str = "written";
pub const TAUGHT: &str = "taught";
pub const LEARNED: &str = "learned";
pub const PACKAGED: &str = "packaged";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Skill {
  pub id: String,
  pub name: String,
  pub description: String,
  pub instructions: String,
  pub source: String,
  pub packaged: bool,
  pub team: bool,
  pub created: i64,
  pub updated: i64,
}

/// Create or, if the name is taken, overwrite the existing skill.
pub async fn save(pool: &Pool, name: &str, description: &str, instructions: &str, source: &str) -> Result<Skill> {
  if let Some(existing) = find(pool, name).await? {
    update(pool, &existing.id, name, description, instructions).await?;
    return get(pool, &existing.id).await;
  }
  let id = newid();
  let t = now();
  sqlx::query(
    "INSERT INTO skills (id, name, description, instructions, source, packaged, created, updated) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(&id)
  .bind(name.trim())
  .bind(description)
  .bind(instructions)
  .bind(source)
  .bind(source == PACKAGED)
  .bind(t)
  .bind(t)
  .execute(pool)
  .await?;
  get(pool, &id).await
}

pub async fn get(pool: &Pool, id: &str) -> Result<Skill> {
  Ok(sqlx::query_as("SELECT * FROM skills WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn find(pool: &Pool, name: &str) -> Result<Option<Skill>> {
  Ok(
    sqlx::query_as("SELECT * FROM skills WHERE lower(name) = lower(?)")
      .bind(name.trim().trim_start_matches('/'))
      .fetch_optional(pool)
      .await?,
  )
}

pub async fn all(pool: &Pool) -> Result<Vec<Skill>> {
  Ok(sqlx::query_as("SELECT * FROM skills ORDER BY name COLLATE NOCASE").fetch_all(pool).await?)
}

/// Skills switched on for one Bot.
pub async fn enabled(pool: &Pool, bot: &str) -> Result<Vec<Skill>> {
  Ok(
    sqlx::query_as(
      "SELECT s.* FROM skills s JOIN bot_skills b ON b.skill_id = s.id WHERE b.bot_id = ? ORDER BY s.name COLLATE NOCASE",
    )
    .bind(bot)
    .fetch_all(pool)
    .await?,
  )
}

pub async fn enable(pool: &Pool, bot: &str, skill: &str, on: bool) -> Result<()> {
  let sql = if on {
    "INSERT OR IGNORE INTO bot_skills (bot_id, skill_id) VALUES (?, ?)"
  } else {
    "DELETE FROM bot_skills WHERE bot_id = ? AND skill_id = ?"
  };
  sqlx::query(sql).bind(bot).bind(skill).execute(pool).await?;
  Ok(())
}

pub async fn update(pool: &Pool, id: &str, name: &str, description: &str, instructions: &str) -> Result<()> {
  sqlx::query("UPDATE skills SET name = ?, description = ?, instructions = ?, updated = ? WHERE id = ?")
    .bind(name.trim())
    .bind(description)
    .bind(instructions)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn set_team(pool: &Pool, id: &str, on: bool) -> Result<()> {
  sqlx::query("UPDATE skills SET team = ? WHERE id = ?").bind(on).bind(id).execute(pool).await?;
  Ok(())
}

pub async fn delete(pool: &Pool, id: &str) -> Result<()> {
  sqlx::query("DELETE FROM skills WHERE id = ?").bind(id).execute(pool).await?;
  Ok(())
}

#[cfg(test)]
#[path = "../tests/skills.rs"]
mod tests;
