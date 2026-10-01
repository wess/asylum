use crate::Pool;
use anyhow::Result;

#[derive(Clone, Debug, Default, PartialEq, sqlx::FromRow)]
pub struct Row {
  pub day: String,
  pub model: String,
  pub bot_id: String,
  pub prompt_tokens: i64,
  pub completion_tokens: i64,
  pub requests: i64,
}

pub async fn record(pool: &Pool, day: &str, model: &str, bot: &str, prompt: i64, completion: i64) -> Result<()> {
  sqlx::query(
    "INSERT INTO usage (day, model, bot_id, prompt_tokens, completion_tokens, requests) VALUES (?, ?, ?, ?, ?, 1)
     ON CONFLICT(day, model, bot_id) DO UPDATE SET
       prompt_tokens = prompt_tokens + excluded.prompt_tokens,
       completion_tokens = completion_tokens + excluded.completion_tokens,
       requests = requests + 1",
  )
  .bind(day)
  .bind(model)
  .bind(bot)
  .bind(prompt)
  .bind(completion)
  .execute(pool)
  .await?;
  Ok(())
}

/// Per-day totals since `since` (a `YYYY-MM-DD`), oldest first.
pub async fn daily(pool: &Pool, since: &str) -> Result<Vec<Row>> {
  Ok(
    sqlx::query_as(
      "SELECT day, '' AS model, '' AS bot_id, SUM(prompt_tokens) AS prompt_tokens,
              SUM(completion_tokens) AS completion_tokens, SUM(requests) AS requests
       FROM usage WHERE day >= ? GROUP BY day ORDER BY day",
    )
    .bind(since)
    .fetch_all(pool)
    .await?,
  )
}

pub async fn by_bot(pool: &Pool, since: &str) -> Result<Vec<Row>> {
  Ok(
    sqlx::query_as(
      "SELECT '' AS day, '' AS model, bot_id, SUM(prompt_tokens) AS prompt_tokens,
              SUM(completion_tokens) AS completion_tokens, SUM(requests) AS requests
       FROM usage WHERE day >= ? GROUP BY bot_id ORDER BY SUM(prompt_tokens + completion_tokens) DESC",
    )
    .bind(since)
    .fetch_all(pool)
    .await?,
  )
}
