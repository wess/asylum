use crate::{newid, now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub const ROLES: [&str; 3] = ["owner", "admin", "member"];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Team {
  pub id: String,
  pub name: String,
  pub created: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct TeamMember {
  pub team_id: String,
  pub email: String,
  pub name: String,
  pub role: String,
  pub created: i64,
}

pub async fn create(pool: &Pool, name: &str) -> Result<Team> {
  let id = newid();
  sqlx::query("INSERT INTO teams (id, name, created) VALUES (?, ?, ?)")
    .bind(&id)
    .bind(name)
    .bind(now())
    .execute(pool)
    .await?;
  Ok(sqlx::query_as("SELECT * FROM teams WHERE id = ?").bind(&id).fetch_one(pool).await?)
}

pub async fn all(pool: &Pool) -> Result<Vec<Team>> {
  Ok(sqlx::query_as("SELECT * FROM teams ORDER BY name").fetch_all(pool).await?)
}

pub async fn rename(pool: &Pool, id: &str, name: &str) -> Result<()> {
  sqlx::query("UPDATE teams SET name = ? WHERE id = ?").bind(name).bind(id).execute(pool).await?;
  Ok(())
}

pub async fn delete(pool: &Pool, id: &str) -> Result<()> {
  sqlx::query("DELETE FROM teams WHERE id = ?").bind(id).execute(pool).await?;
  Ok(())
}

pub async fn members(pool: &Pool, team: &str) -> Result<Vec<TeamMember>> {
  Ok(
    sqlx::query_as("SELECT * FROM team_members WHERE team_id = ? ORDER BY role, email")
      .bind(team)
      .fetch_all(pool)
      .await?,
  )
}

pub async fn invite(pool: &Pool, team: &str, email: &str, name: &str, role: &str) -> Result<()> {
  let role = if ROLES.contains(&role) { role } else { "member" };
  sqlx::query("INSERT OR REPLACE INTO team_members (team_id, email, name, role, created) VALUES (?, ?, ?, ?, ?)")
    .bind(team)
    .bind(email.trim().to_lowercase())
    .bind(name)
    .bind(role)
    .bind(now())
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn remove(pool: &Pool, team: &str, email: &str) -> Result<()> {
  sqlx::query("DELETE FROM team_members WHERE team_id = ? AND email = ?")
    .bind(team)
    .bind(email)
    .execute(pool)
    .await?;
  Ok(())
}
