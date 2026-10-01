use crate::{newid, now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Bot {
  pub id: String,
  pub name: String,
  pub label: String,
  pub description: String,
  pub avatar: String,
  pub color: String,
  pub kind: String,
  pub notifications: bool,
  pub pinned: bool,
  pub hidden: bool,
  pub primary_bot: bool,
  pub position: i64,
  pub section_id: Option<String>,
  pub team_id: Option<String>,
  pub owner: String,
  pub creator_id: Option<String>,
  pub published: bool,
  pub required: bool,
  pub status: String,
  pub created: i64,
  pub updated: i64,
  pub active: i64,
  pub provider: String,
  pub model: String,
}

/// The editable part of a profile.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
  pub name: String,
  pub label: String,
  pub description: String,
  pub avatar: String,
  pub color: String,
}

pub const PERSONAL: &str = "personal";
pub const TEAM: &str = "team";
pub const HELPER: &str = "helper";
pub const SYSTEM: &str = "system";

/// Status values shown by the animated avatar.
pub const IDLE: &str = "idle";
pub const WORKING: &str = "working";
pub const WAITING: &str = "waiting";
pub const DONE: &str = "done";

impl Bot {
  pub fn is_team(&self) -> bool {
    self.kind == TEAM
  }

  pub fn profile(&self) -> Profile {
    Profile {
      name: self.name.clone(),
      label: self.label.clone(),
      description: self.description.clone(),
      avatar: self.avatar.clone(),
      color: self.color.clone(),
    }
  }
}

const ORDER: &str = "ORDER BY pinned DESC, position ASC, active DESC, created ASC";

pub async fn create(pool: &Pool, p: &Profile) -> Result<Bot> {
  create_kind(pool, p, PERSONAL, None).await
}

pub async fn create_kind(pool: &Pool, p: &Profile, kind: &str, creator: Option<&str>) -> Result<Bot> {
  let id = newid();
  let t = now();
  let position: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 1 FROM bots")
    .fetch_one(pool)
    .await?;
  sqlx::query(
    "INSERT INTO bots (id, name, label, description, avatar, color, kind, creator_id, position, created, updated, active)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
  )
  .bind(&id)
  .bind(&p.name)
  .bind(&p.label)
  .bind(&p.description)
  .bind(&p.avatar)
  .bind(&p.color)
  .bind(kind)
  .bind(creator)
  .bind(position)
  .bind(t)
  .bind(t)
  .bind(t)
  .execute(pool)
  .await?;
  get(pool, &id).await
}

pub async fn get(pool: &Pool, id: &str) -> Result<Bot> {
  Ok(
    sqlx::query_as("SELECT * FROM bots WHERE id = ?")
      .bind(id)
      .fetch_one(pool)
      .await?,
  )
}

pub async fn find(pool: &Pool, name: &str) -> Result<Option<Bot>> {
  Ok(
    sqlx::query_as("SELECT * FROM bots WHERE lower(name) = lower(?) LIMIT 1")
      .bind(name.trim().trim_start_matches('@'))
      .fetch_optional(pool)
      .await?,
  )
}

/// Every Bot, sidebar order. Hidden ones are included; the caller filters.
pub async fn list(pool: &Pool) -> Result<Vec<Bot>> {
  Ok(
    sqlx::query_as(&format!("SELECT * FROM bots {ORDER}"))
      .fetch_all(pool)
      .await?,
  )
}

pub async fn update(pool: &Pool, id: &str, p: &Profile) -> Result<Bot> {
  sqlx::query(
    "UPDATE bots SET name = ?, label = ?, description = ?, avatar = ?, color = ?, updated = ? WHERE id = ?",
  )
  .bind(&p.name)
  .bind(&p.label)
  .bind(&p.description)
  .bind(&p.avatar)
  .bind(&p.color)
  .bind(now())
  .bind(id)
  .execute(pool)
  .await?;
  get(pool, id).await
}

async fn set_flag(pool: &Pool, id: &str, column: &str, on: bool) -> Result<()> {
  sqlx::query(&format!(
    "UPDATE bots SET {column} = ?, updated = ? WHERE id = ?"
  ))
  .bind(on)
  .bind(now())
  .bind(id)
  .execute(pool)
  .await?;
  Ok(())
}

pub async fn pin(pool: &Pool, id: &str, on: bool) -> Result<()> {
  set_flag(pool, id, "pinned", on).await
}

pub async fn hide(pool: &Pool, id: &str, on: bool) -> Result<()> {
  set_flag(pool, id, "hidden", on).await
}

/// Only one Bot is the primary (starred) Bot at a time.
pub async fn set_primary(pool: &Pool, id: &str) -> Result<()> {
  sqlx::query("UPDATE bots SET primary_bot = (id = ?)").bind(id).execute(pool).await?;
  Ok(())
}

pub async fn primary(pool: &Pool) -> Result<Option<Bot>> {
  Ok(
    sqlx::query_as("SELECT * FROM bots WHERE primary_bot = 1 LIMIT 1")
      .fetch_optional(pool)
      .await?,
  )
}

pub async fn set_notifications(pool: &Pool, id: &str, on: bool) -> Result<()> {
  set_flag(pool, id, "notifications", on).await
}

pub async fn set_section(pool: &Pool, id: &str, section: Option<&str>) -> Result<()> {
  sqlx::query("UPDATE bots SET section_id = ?, updated = ? WHERE id = ?")
    .bind(section)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

/// Pin a Bot to a provider profile and model (empty = the defaults).
pub async fn set_model(pool: &Pool, id: &str, provider: &str, model: &str) -> Result<()> {
  sqlx::query("UPDATE bots SET provider = ?, model = ?, updated = ? WHERE id = ?")
    .bind(provider)
    .bind(model)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn set_owner(pool: &Pool, id: &str, owner: &str) -> Result<()> {
  sqlx::query("UPDATE bots SET owner = ? WHERE id = ?").bind(owner).bind(id).execute(pool).await?;
  Ok(())
}

pub async fn set_published(pool: &Pool, id: &str, on: bool) -> Result<()> {
  set_flag(pool, id, "published", on).await
}

pub async fn set_kind(pool: &Pool, id: &str, kind: &str) -> Result<()> {
  sqlx::query("UPDATE bots SET kind = ?, updated = ? WHERE id = ?")
    .bind(kind)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn by_kind(pool: &Pool, kind: &str) -> Result<Vec<Bot>> {
  Ok(
    sqlx::query_as(&format!("SELECT * FROM bots WHERE kind = ? {ORDER}"))
      .bind(kind)
      .fetch_all(pool)
      .await?,
  )
}

pub async fn count(pool: &Pool) -> Result<i64> {
  Ok(sqlx::query_scalar("SELECT COUNT(*) FROM bots").fetch_one(pool).await?)
}

pub async fn set_status(pool: &Pool, id: &str, status: &str) -> Result<()> {
  sqlx::query("UPDATE bots SET status = ?, active = ? WHERE id = ?")
    .bind(status)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn set_team(pool: &Pool, id: &str, team: Option<&str>) -> Result<()> {
  sqlx::query("UPDATE bots SET team_id = ?, updated = ? WHERE id = ?")
    .bind(team)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

/// Persist a drag-reordered sidebar: `ids` in their new order.
pub async fn reorder(pool: &Pool, ids: &[String]) -> Result<()> {
  let mut tx = pool.begin().await?;
  for (i, id) in ids.iter().enumerate() {
    sqlx::query("UPDATE bots SET position = ? WHERE id = ?")
      .bind(i as i64)
      .bind(id)
      .execute(&mut *tx)
      .await?;
  }
  tx.commit().await?;
  Ok(())
}

pub async fn delete(pool: &Pool, id: &str) -> Result<()> {
  sqlx::query("DELETE FROM bots WHERE id = ?")
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

/// A name no other Bot has: `base`, then `base 2`, `base 3`, ...
pub async fn unique_name(pool: &Pool, base: &str) -> Result<String> {
  let names: Vec<String> = sqlx::query_scalar("SELECT lower(name) FROM bots")
    .fetch_all(pool)
    .await?;
  Ok(next_name(base, &names))
}

pub fn next_name(base: &str, taken: &[String]) -> String {
  let lower = base.to_lowercase();
  if !taken.contains(&lower) {
    return base.to_string();
  }
  (2..)
    .map(|n| format!("{base} {n}"))
    .find(|n| !taken.contains(&n.to_lowercase()))
    .unwrap_or_default()
}

#[cfg(test)]
#[path = "../tests/bots.rs"]
mod tests;

/// Added by the admin's policy: always shown, never deleted.
pub async fn set_required(pool: &Pool, id: &str, on: bool) -> Result<()> {
  set_flag(pool, id, "required", on).await?;
  if on {
    set_flag(pool, id, "hidden", false).await?;
  }
  Ok(())
}
