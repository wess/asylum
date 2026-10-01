use super::*;

#[test]
fn back_and_forward() {
  let mut h = History::default();
  h.visit(Some("a"), "b");
  h.visit(Some("b"), "c");
  assert_eq!(h.back(Some("c")).as_deref(), Some("b"));
  assert_eq!(h.back(Some("b")).as_deref(), Some("a"));
  assert_eq!(h.back(Some("a")), None);
  assert_eq!(h.forward(Some("a")).as_deref(), Some("b"));
  h.visit(Some("b"), "d");
  assert_eq!(h.forward(Some("d")), None);
}
