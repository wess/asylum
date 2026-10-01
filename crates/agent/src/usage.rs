//! Usage metering. Tokens are recorded per day, model, and Bot. Included
//! usage resets weekly (Monday); past it, Keep going continues up to the
//! monthly limit, which is soft: a running Bot finishes its turn.

use crate::event::Event;
use crate::runtime::Runtime;
use anyhow::{bail, Result};
use chrono::{Datelike, Duration, NaiveDate};

pub fn today(rt: &Runtime) -> NaiveDate {
  chrono::Utc::now().with_timezone(&rt.tz()).date_naive()
}

pub fn week_start(d: NaiveDate) -> NaiveDate {
  d - Duration::days(d.weekday().num_days_from_monday() as i64)
}

pub fn month_start(d: NaiveDate) -> NaiveDate {
  d.with_day(1).unwrap_or(d)
}

pub async fn record(rt: &Runtime, model: &str, bot: &str, prompt: i64, completion: i64) {
  let day = today(rt).format("%Y-%m-%d").to_string();
  let _ = store::usage::record(&rt.pool, &day, model, bot, prompt, completion).await;
  rt.emit(Event::Usage);
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Summary {
  pub week: u64,
  pub weekly_limit: u64,
  pub month_over: u64,
  pub monthly_limit: u64,
  pub on_demand: bool,
}

impl Summary {
  pub fn fraction(&self) -> f32 {
    if self.weekly_limit == 0 {
      return 0.0;
    }
    (self.week as f32 / self.weekly_limit as f32).min(1.0)
  }

  pub fn blocked(&self) -> bool {
    if self.week < self.weekly_limit {
      return false;
    }
    !self.on_demand || self.month_over >= self.monthly_limit
  }
}

pub async fn summary(rt: &Runtime) -> Result<Summary> {
  let s = rt.settings();
  let today = today(rt);
  let week = week_start(today).format("%Y-%m-%d").to_string();
  let month = month_start(today).format("%Y-%m-%d").to_string();
  let week_total: i64 = store::usage::daily(&rt.pool, &week).await?.iter().map(|r| r.prompt_tokens + r.completion_tokens).sum();
  // Usage past the weekly limit this month: whatever went past each week's allowance.
  let days = store::usage::daily(&rt.pool, &month).await?;
  let mut over = 0i64;
  let mut acc: std::collections::BTreeMap<NaiveDate, i64> = Default::default();
  for d in days {
    if let Ok(day) = NaiveDate::parse_from_str(&d.day, "%Y-%m-%d") {
      *acc.entry(week_start(day)).or_default() += d.prompt_tokens + d.completion_tokens;
    }
  }
  for (_, total) in acc {
    over += (total - s.weekly_limit as i64).max(0);
  }
  Ok(Summary {
    week: week_total.max(0) as u64,
    weekly_limit: s.weekly_limit,
    month_over: over.max(0) as u64,
    monthly_limit: s.monthly_limit,
    on_demand: s.on_demand,
  })
}

pub async fn check(rt: &Runtime) -> Result<()> {
  let s = summary(rt).await?;
  if s.blocked() {
    bail!("You've reached your usage limit. Raise it or turn on Keep going in Settings → Usage.");
  }
  Ok(())
}

#[cfg(test)]
#[path = "../tests/usage.rs"]
mod tests;
