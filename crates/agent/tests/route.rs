use super::*;

fn members() -> Vec<(String, String)> {
  vec![("1".into(), "Scout".into()), ("2".into(), "Chief of Staff".into()), ("3".into(), "Scout Jr".into())]
}

#[test]
fn mentions() {
  assert_eq!(targets("hey @scout jr can you", &members()), Target::Named(vec!["3".into()]));
  assert_eq!(targets("@Scout and @chief of staff", &members()), Target::Named(vec!["1".into(), "2".into()]));
  assert_eq!(targets("@everyone standup", &members()), Target::Everyone);
  assert_eq!(targets("no mention", &members()), Target::Undecided);
  assert_eq!(targets("mail me@scoutx.com", &members()), Target::Undecided);
}

#[test]
fn pick() {
  assert_eq!(parse_pick("Chief of Staff should answer", &members()), vec!["2".to_string()]);
}
