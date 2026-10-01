//! Secret values never reach the model: any occurrence in tool output is
//! replaced with `[REDACTED]`. Short values are skipped to avoid shredding
//! ordinary text.

pub const MARK: &str = "[REDACTED]";

pub fn redact(text: &str, secrets: &[String]) -> String {
  let mut out = text.to_string();
  let mut values: Vec<&String> = secrets.iter().filter(|s| s.len() >= 4).collect();
  values.sort_by_key(|s| std::cmp::Reverse(s.len()));
  for v in values {
    if out.contains(v.as_str()) {
      out = out.replace(v.as_str(), MARK);
    }
  }
  out
}

#[cfg(test)]
#[path = "../tests/redact.rs"]
mod tests;
