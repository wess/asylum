//! Conversation Insights: what the Bots have been doing over a period —
//! conversations, messages, tool use and outcomes, approvals, and tokens.

use crate::runtime::Runtime;
use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Insights {
  pub since: i64,
  pub messages: i64,
  pub conversations: i64,
  pub actions: i64,
  /// Outcome → count (done, error, denied, user-denied, expired).
  pub outcomes: BTreeMap<String, i64>,
  /// The most used tools, most first.
  pub tools: Vec<(String, i64)>,
  /// Bot name → actions.
  pub bots: Vec<(String, i64)>,
}

pub async fn since(rt: &Runtime, since: i64) -> Result<Insights> {
  let pool = &rt.pool;
  let (messages, conversations): (i64, i64) = sqlx_counts(pool, since).await?;
  let actions = store::events::list(pool, store::events::ACTION, since, 100_000).await?;
  let names: std::collections::HashMap<String, String> = store::bots::list(pool).await?.into_iter().map(|b| (b.id, b.name)).collect();
  let mut outcomes = BTreeMap::new();
  let mut tools: BTreeMap<String, i64> = BTreeMap::new();
  let mut bots: BTreeMap<String, i64> = BTreeMap::new();
  for a in &actions {
    *outcomes.entry(a.outcome.clone()).or_default() += 1;
    *tools.entry(a.name.clone()).or_default() += 1;
    let who = a.bot_id.as_ref().and_then(|b| names.get(b)).cloned().unwrap_or_default();
    *bots.entry(who).or_default() += 1;
  }
  let top = |m: BTreeMap<String, i64>| {
    let mut v: Vec<(String, i64)> = m.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.truncate(10);
    v
  };
  Ok(Insights { since, messages, conversations, actions: actions.len() as i64, outcomes, tools: top(tools), bots: top(bots) })
}

async fn sqlx_counts(pool: &store::Pool, since: i64) -> Result<(i64, i64)> {
  store::messages::activity(pool, since).await
}
