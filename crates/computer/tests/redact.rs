use super::*;

#[test]
fn replaces_longest_first_and_skips_short() {
  let secrets = vec!["abc".into(), "sk-12345".into(), "sk-12345678".into()];
  assert_eq!(redact("key sk-12345678 abc", &secrets), "key [REDACTED] abc");
}
