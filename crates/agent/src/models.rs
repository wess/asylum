//! There is no model picker: the runtime routes to the best available
//! model. `pick` chooses from what the account can use, preferring the
//! configured name.

pub fn pick(available: &[String], preferred: &str, fallbacks: &[&str]) -> String {
  if available.is_empty() || available.iter().any(|m| m == preferred) {
    return preferred.to_string();
  }
  for f in fallbacks {
    if let Some(m) = available.iter().find(|m| m.as_str() == *f) {
      return m.clone();
    }
  }
  let mut groks: Vec<&String> = available
    .iter()
    .filter(|m| m.starts_with("grok") && !m.contains("image") && !m.contains("vision") && !m.contains("imagine"))
    .collect();
  groks.sort_by_key(|m| std::cmp::Reverse(rank(m)));
  groks.first().map(|m| m.to_string()).unwrap_or_else(|| preferred.to_string())
}

/// Higher is newer/stronger: version digits first, then non-mini/fast.
fn rank(m: &str) -> (u32, u32, bool) {
  let digits: Vec<u32> = m
    .split(|c: char| !c.is_ascii_digit())
    .filter_map(|s| s.parse().ok())
    .filter(|n| *n < 100)
    .collect();
  let major = digits.first().copied().unwrap_or(0);
  let minor = digits.get(1).copied().unwrap_or(0);
  (major, minor, !(m.contains("mini") || m.contains("fast")))
}

pub const MAIN: [&str; 4] = ["grok-4", "grok-4-latest", "grok-4-0709", "grok-3"];
pub const FAST: [&str; 5] = ["grok-4-fast", "grok-4-fast-non-reasoning", "grok-3-mini", "grok-3-mini-fast", "grok-4"];

#[cfg(test)]
#[path = "../tests/models.rs"]
mod tests;
