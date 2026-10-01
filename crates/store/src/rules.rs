//! Auto-review rules: natural-language "Ask first" and "Allow automatically"
//! rules. Ask-first wins on conflict. Locked rows come from a team admin.

use crate::{newid, now, Pool};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const ASK: &str = "ask";
pub const ALLOW: &str = "allow";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Rule {
  pub id: String,
  pub kind: String,
  pub text: String,
  pub locked: bool,
  pub created: i64,
}

pub async fn add(pool: &Pool, kind: &str, text: &str, locked: bool) -> Result<Rule> {
  if kind != ASK && kind != ALLOW {
    bail!("rule kind must be ask or allow");
  }
  let id = newid();
  sqlx::query("INSERT INTO rules (id, kind, text, locked, created) VALUES (?, ?, ?, ?, ?)")
    .bind(&id)
    .bind(kind)
    .bind(text.trim())
    .bind(locked)
    .bind(now())
    .execute(pool)
    .await?;
  Ok(sqlx::query_as("SELECT * FROM rules WHERE id = ?").bind(&id).fetch_one(pool).await?)
}

pub async fn all(pool: &Pool) -> Result<Vec<Rule>> {
  Ok(sqlx::query_as("SELECT * FROM rules ORDER BY locked DESC, kind, created").fetch_all(pool).await?)
}

pub async fn update(pool: &Pool, id: &str, kind: &str, text: &str) -> Result<()> {
  let n = sqlx::query("UPDATE rules SET kind = ?, text = ? WHERE id = ? AND locked = 0")
    .bind(kind)
    .bind(text.trim())
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();
  if n == 0 {
    bail!("Required by your admin. You can't edit or delete this rule.");
  }
  Ok(())
}

pub async fn delete(pool: &Pool, id: &str) -> Result<()> {
  let n = sqlx::query("DELETE FROM rules WHERE id = ? AND locked = 0")
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();
  if n == 0 {
    bail!("Required by your admin. You can't edit or delete this rule.");
  }
  Ok(())
}

/// Make the locked rules exactly `want` (kind, text), keeping matching rows.
pub async fn sync_locked(pool: &Pool, want: &[(String, String)]) -> Result<()> {
  let have: Vec<Rule> = sqlx::query_as("SELECT * FROM rules WHERE locked = 1").fetch_all(pool).await?;
  for r in &have {
    if !want.iter().any(|(k, t)| *k == r.kind && t.trim() == r.text) {
      sqlx::query("DELETE FROM rules WHERE id = ?").bind(&r.id).execute(pool).await?;
    }
  }
  for (k, t) in want {
    if !have.iter().any(|r| r.kind == *k && r.text == t.trim()) {
      add(pool, k, t, true).await?;
    }
  }
  Ok(())
}
