//! Who answers in a group: `@Name` targets one Bot, several mentions target
//! several, `@everyone` targets all, and a plain message lets the Bots decide.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
  Everyone,
  Named(Vec<String>),
  Undecided,
}

/// Resolve mentions against member (id, name) pairs. Names may contain
/// spaces, so the longest name that follows an `@` wins.
pub fn targets(text: &str, members: &[(String, String)]) -> Target {
  let lower = text.to_lowercase();
  if lower.contains("@everyone") || lower.contains("@all ") || lower.ends_with("@all") {
    return Target::Everyone;
  }
  let mut sorted: Vec<&(String, String)> = members.iter().collect();
  sorted.sort_by_key(|(_, n)| std::cmp::Reverse(n.len()));
  let mut hits = Vec::new();
  for (i, _) in lower.match_indices('@') {
    let rest = &lower[i + 1..];
    if let Some((id, _)) = sorted.iter().find(|(_, n)| {
      let n = n.to_lowercase();
      rest.starts_with(&n)
        && rest[n.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric())
    }) {
      if !hits.contains(id) {
        hits.push(id.clone());
      }
    }
  }
  if hits.is_empty() {
    Target::Undecided
  } else {
    Target::Named(hits)
  }
}

/// Names of the members in `answer` (the model's pick), in member order.
pub fn parse_pick(answer: &str, members: &[(String, String)]) -> Vec<String> {
  let lower = answer.to_lowercase();
  members
    .iter()
    .filter(|(_, n)| lower.contains(&n.to_lowercase()))
    .map(|(id, _)| id.clone())
    .collect()
}

#[cfg(test)]
#[path = "../tests/route.rs"]
mod tests;
