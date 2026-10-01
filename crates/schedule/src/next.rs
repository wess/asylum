use crate::cron::Cron;
use anyhow::{bail, Result};
use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use chrono_tz::Tz;

/// The first minute strictly after `after_ms` (unix millis) that matches the
/// expression in `tz`, as unix millis.
pub fn next_cron(expr: &str, after_ms: i64, tz: Tz) -> Result<i64> {
  let cron = Cron::parse(expr)?;
  let start = Utc
    .timestamp_millis_opt(after_ms)
    .single()
    .unwrap_or_else(Utc::now)
    .with_timezone(&tz);
  let mut t = (start + Duration::minutes(1))
    .with_second(0)
    .and_then(|t| t.with_nanosecond(0))
    .unwrap_or(start);
  // Five years covers any satisfiable expression (Feb 29 on a weekday).
  let limit = start + Duration::days(366 * 5);
  while t < limit {
    if !cron.day_matches(t.day(), t.month(), t.weekday().num_days_from_sunday()) {
      t = next_midnight(t, tz);
      continue;
    }
    if !cron.hours.contains(&t.hour()) {
      t = next_hour(t);
      continue;
    }
    if cron.minutes.contains(&t.minute()) {
      return Ok(t.with_timezone(&Utc).timestamp_millis());
    }
    t += Duration::minutes(1);
  }
  bail!("this schedule never runs")
}

fn next_midnight(t: DateTime<Tz>, tz: Tz) -> DateTime<Tz> {
  let date = t.date_naive() + Duration::days(1);
  let naive = date.and_hms_opt(0, 0, 0).expect("midnight");
  tz.from_local_datetime(&naive)
    .earliest()
    .unwrap_or_else(|| tz.from_utc_datetime(&naive))
}

fn next_hour(t: DateTime<Tz>) -> DateTime<Tz> {
  let base = t.with_minute(0).unwrap_or(t);
  base + Duration::hours(1)
}

/// Next run for any timed trigger. Event triggers have none.
pub fn next_run(trigger: &str, schedule: &str, after_ms: i64, tz: Tz) -> Result<Option<i64>> {
  match trigger {
    "schedule" => next_cron(schedule, after_ms, tz).map(Some),
    "interval" => {
      let minutes: i64 = schedule.trim().parse().unwrap_or(0);
      if minutes <= 0 {
        bail!("an interval is a positive number of minutes");
      }
      Ok(Some(after_ms + minutes * 60_000))
    }
    _ => Ok(None),
  }
}

#[cfg(test)]
#[path = "../tests/next.rs"]
mod tests;
