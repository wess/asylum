use super::*;

fn v(xs: &[&str]) -> Vec<String> {
  xs.iter().map(|s| s.to_string()).collect()
}

#[test]
fn keeps_preferred_when_available_or_unknown() {
  assert_eq!(pick(&[], "grok-4", &MAIN), "grok-4");
  assert_eq!(pick(&v(&["grok-4", "grok-3"]), "grok-4", &MAIN), "grok-4");
}

#[test]
fn falls_back_then_ranks() {
  assert_eq!(pick(&v(&["grok-3", "grok-2-image"]), "grok-4", &MAIN), "grok-3");
  assert_eq!(pick(&v(&["grok-5-mini", "grok-5", "grok-2-image-1212"]), "grok-9", &[]), "grok-5");
}
