use super::*;

#[test]
fn error_result_fails() {
  let mut l = Lines::default();
  l.push(r#"{"type":"result","is_error":true,"subtype":"error_max_turns","result":""}"#);
  assert!(l.finish().unwrap_err().to_string().contains("error_max_turns"));
}

#[test]
fn result_without_deltas_is_the_answer() {
  let mut l = Lines::default();
  assert!(l.push("not json").is_empty());
  l.push(r#"{"type":"result","result":"done","usage":{"input_tokens":1,"cache_read_input_tokens":2,"output_tokens":3}}"#);
  let (text, usage) = l.finish().unwrap();
  assert_eq!(text, "done");
  assert_eq!(usage.unwrap().prompt_tokens, 3);
}
