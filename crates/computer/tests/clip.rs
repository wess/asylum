use super::*;

#[test]
fn short_text_untouched() {
  assert_eq!(clip("abc", 10), "abc");
}

#[test]
fn long_text_keeps_head_and_tail() {
  let s = "é".repeat(100);
  let c = clip(&s, 40);
  assert!(c.starts_with('é'));
  assert!(c.ends_with('é'));
  assert!(c.contains("elided"));
}
