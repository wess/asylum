//! A schedule as the sentence shown under "When to run".

use crate::cron::Cron;

pub fn describe(trigger: &str, schedule: &str, filter: &str) -> String {
  match trigger {
    "schedule" => Cron::parse(schedule)
      .map(|c| cron_sentence(&c))
      .unwrap_or_else(|_| format!("On schedule {schedule}")),
    "interval" => {
      let m: u64 = schedule.trim().parse().unwrap_or(0);
      match m {
        0 => "Never".into(),
        m if m % 1440 == 0 => plural(m / 1440, "day"),
        m if m % 60 == 0 => plural(m / 60, "hour"),
        m => plural(m, "minute"),
      }
    }
    "webhook" => "When its webhook receives a POST".into(),
    "email" => with_filter("When an email arrives", filter),
    "slack" => with_filter("When a Slack event happens", filter),
    "github" => with_filter("When a GitHub event happens", filter),
    "linear" => with_filter("When a Linear event happens", filter),
    "sentry" => with_filter("When Sentry reports an issue", filter),
    "pagerduty" => with_filter("When PagerDuty raises an incident", filter),
    t => format!("On {t}"),
  }
}

fn with_filter(base: &str, filter: &str) -> String {
  let v: serde_like::Map = serde_like::parse(filter);
  if v.is_empty() {
    return base.to_string();
  }
  format!("{base} ({})", v.join(", "))
}

/// Tiny key=value extraction from the filter JSON without pulling serde in.
mod serde_like {
  pub type Map = Vec<String>;

  pub fn parse(json: &str) -> Map {
    let inner = json.trim().trim_start_matches('{').trim_end_matches('}');
    inner
      .split(',')
      .filter_map(|pair| {
        let (k, v) = pair.split_once(':')?;
        let k = k.trim().trim_matches('"');
        let v = v.trim().trim_matches('"');
        (!k.is_empty() && !v.is_empty()).then(|| format!("{k}: {v}"))
      })
      .collect()
  }
}

fn plural(n: u64, unit: &str) -> String {
  if n == 1 {
    format!("Every {unit}")
  } else {
    format!("Every {n} {unit}s")
  }
}

fn clock(h: u32, m: u32) -> String {
  let (h12, ampm) = match h {
    0 => (12, "AM"),
    1..=11 => (h, "AM"),
    12 => (12, "PM"),
    _ => (h - 12, "PM"),
  };
  format!("{h12}:{m:02} {ampm}")
}

const DAY_NAMES: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTH_NAMES: [&str; 12] = [
  "January", "February", "March", "April", "May", "June", "July", "August", "September", "October",
  "November", "December",
];

fn ordinal(n: u32) -> String {
  let suffix = match (n % 10, n % 100) {
    (1, 11) | (2, 12) | (3, 13) => "th",
    (1, _) => "st",
    (2, _) => "nd",
    (3, _) => "rd",
    _ => "th",
  };
  format!("{n}{suffix}")
}

fn list(items: &[String]) -> String {
  match items.len() {
    0 => String::new(),
    1 => items[0].clone(),
    n => format!("{} and {}", items[..n - 1].join(", "), items[n - 1]),
  }
}

fn cron_sentence(c: &Cron) -> String {
  let times = if c.minutes.len() == 60 && c.hours.len() == 24 {
    "every minute".to_string()
  } else if c.hours.len() == 24 && c.minutes.len() == 1 {
    if c.minutes[0] == 0 {
      "every hour".to_string()
    } else {
      format!("every hour at :{:02}", c.minutes[0])
    }
  } else if c.hours.len() == 24 {
    step_phrase(&c.minutes, 60, "minute").unwrap_or_else(|| format!("at minutes {}", join_nums(&c.minutes)))
  } else if c.minutes.len() == 1 && c.hours.len() <= 4 {
    let ts: Vec<String> = c.hours.iter().map(|h| clock(*h, c.minutes[0])).collect();
    format!("at {}", list(&ts))
  } else if c.minutes.len() == 1 {
    let every = step_phrase(&c.hours, 24, "hour").unwrap_or_else(|| format!("at hours {}", join_nums(&c.hours)));
    if c.minutes[0] == 0 {
      every
    } else {
      format!("{every} at :{:02}", c.minutes[0])
    }
  } else {
    format!("at minutes {} of hours {}", join_nums(&c.minutes), join_nums(&c.hours))
  };

  let days = if c.any_day && c.any_weekday {
    "Every day".to_string()
  } else if c.any_day {
    match c.weekdays.as_slice() {
      [1, 2, 3, 4, 5] => "Every weekday".into(),
      [0, 6] => "Every weekend day".into(),
      ws => format!(
        "Every {}",
        list(&ws.iter().map(|w| DAY_NAMES[*w as usize].to_string()).collect::<Vec<_>>())
      ),
    }
  } else {
    let ds: Vec<String> = c.days.iter().map(|d| ordinal(*d)).collect();
    format!("On the {} of the month", list(&ds))
  };

  let months = if c.months.len() == 12 {
    String::new()
  } else {
    format!(
      " in {}",
      list(&c.months.iter().map(|m| MONTH_NAMES[*m as usize - 1].to_string()).collect::<Vec<_>>())
    )
  };
  format!("{days}{months} {times}")
}

fn step_phrase(vals: &[u32], span: u32, unit: &str) -> Option<String> {
  if vals.len() < 2 || vals[0] != 0 {
    return None;
  }
  let step = vals[1] - vals[0];
  let even = vals.windows(2).all(|w| w[1] - w[0] == step) && span.is_multiple_of(step) && vals.len() as u32 == span / step;
  even.then(|| format!("every {step} {unit}s"))
}

fn join_nums(v: &[u32]) -> String {
  v.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
#[path = "../tests/describe.rs"]
mod tests;
