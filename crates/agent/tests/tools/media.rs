use super::*;

#[test]
fn voice_memo_only_when_voice_is_on() {
  let names = |v: bool| defs(v).into_iter().map(|d| d.function.name).collect::<Vec<_>>();
  assert!(names(false).contains(&"generate_image".to_string()));
  assert!(!names(false).contains(&"voice_memo".to_string()));
  assert!(names(true).contains(&"voice_memo".to_string()));
}
