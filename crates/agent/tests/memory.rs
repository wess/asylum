use super::*;

#[test]
fn parses_items() {
  let t = "Sure:\n[{\"kind\":\"preference\",\"content\":\"Likes short replies\"},{\"kind\":\"fact\",\"content\":\"\"}]";
  assert_eq!(parse(t), vec![("preference".to_string(), "Likes short replies".to_string())]);
  assert!(parse("nothing").is_empty());
}
