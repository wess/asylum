//! Five-field cron: minute hour day-of-month month day-of-week, with `*`,
//! lists, ranges, steps, month/day names, and 7 as Sunday. When both day
//! fields are restricted a day matches either (standard cron semantics).

use anyhow::{anyhow, bail, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cron {
  pub minutes: Vec<u32>,
  pub hours: Vec<u32>,
  pub days: Vec<u32>,
  pub months: Vec<u32>,
  pub weekdays: Vec<u32>,
  pub any_day: bool,
  pub any_weekday: bool,
}

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const DAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

impl Cron {
  pub fn parse(expr: &str) -> Result<Self> {
    let expr = match expr.trim() {
      "@hourly" => "0 * * * *",
      "@daily" | "@midnight" => "0 0 * * *",
      "@weekly" => "0 0 * * 0",
      "@monthly" => "0 0 1 * *",
      "@yearly" | "@annually" => "0 0 1 1 *",
      e => e,
    };
    let f: Vec<&str> = expr.split_whitespace().collect();
    if f.len() != 5 {
      bail!("a schedule needs five fields: minute hour day month weekday");
    }
    let mut weekdays = field(f[4], 0, 7, &DAYS)?;
    for d in weekdays.iter_mut() {
      if *d == 7 {
        *d = 0;
      }
    }
    weekdays.sort_unstable();
    weekdays.dedup();
    Ok(Self {
      minutes: field(f[0], 0, 59, &[])?,
      hours: field(f[1], 0, 23, &[])?,
      days: field(f[2], 1, 31, &[])?,
      months: field(f[3], 1, 12, &MONTHS)?,
      weekdays,
      any_day: f[2] == "*" || f[2] == "?",
      any_weekday: f[4] == "*" || f[4] == "?",
    })
  }

  /// Whether a calendar day (1-based day, month; weekday 0 = Sunday) runs.
  pub fn day_matches(&self, day: u32, month: u32, weekday: u32) -> bool {
    if !self.months.contains(&month) {
      return false;
    }
    let d = self.days.contains(&day);
    let w = self.weekdays.contains(&weekday);
    match (self.any_day, self.any_weekday) {
      (true, true) => true,
      (false, true) => d,
      (true, false) => w,
      (false, false) => d || w,
    }
  }
}

fn value(s: &str, names: &[&str], base: u32) -> Result<u32> {
  if let Ok(n) = s.parse() {
    return Ok(n);
  }
  let lower = s.to_lowercase();
  names
    .iter()
    .position(|n| lower.starts_with(n))
    .map(|i| i as u32 + base)
    .ok_or_else(|| anyhow!("\"{s}\" is not a valid schedule value"))
}

fn field(spec: &str, min: u32, max: u32, names: &[&str]) -> Result<Vec<u32>> {
  let base = if names.len() == 12 { 1 } else { 0 };
  let mut out = Vec::new();
  for part in spec.split(',') {
    let (range, step) = match part.split_once('/') {
      Some((r, s)) => (r, s.parse::<u32>().map_err(|_| anyhow!("bad step in \"{part}\""))?),
      None => (part, 1),
    };
    if step == 0 {
      bail!("a step of 0 never runs");
    }
    let (lo, hi) = match range {
      "*" | "?" => (min, max),
      r => match r.split_once('-') {
        Some((a, b)) => (value(a, names, base)?, value(b, names, base)?),
        None => {
          let v = value(r, names, base)?;
          (v, if part.contains('/') { max } else { v })
        }
      },
    };
    if lo < min || hi > max || lo > hi {
      bail!("\"{part}\" is outside {min}-{max}");
    }
    out.extend((lo..=hi).step_by(step as usize));
  }
  out.sort_unstable();
  out.dedup();
  Ok(out)
}

#[cfg(test)]
#[path = "../tests/cron.rs"]
mod tests;
