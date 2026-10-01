use crate::Pool;
use anyhow::Result;

pub async fn get(pool: &Pool, key: &str) -> Result<Option<String>> {
  Ok(sqlx::query_scalar("SELECT value FROM state WHERE key = ?").bind(key).fetch_optional(pool).await?)
}

pub async fn set(pool: &Pool, key: &str, value: &str) -> Result<()> {
  sqlx::query("INSERT INTO state (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn remove(pool: &Pool, key: &str) -> Result<()> {
  sqlx::query("DELETE FROM state WHERE key = ?").bind(key).execute(pool).await?;
  Ok(())
}
