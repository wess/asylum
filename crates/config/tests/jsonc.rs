use super::*;

#[test]
fn strips_comments_and_trailing_commas() {
  let text = "{\n // note\n \"a\": 1, /* x */\n \"b\": [1,2,],\n}";
  let v: serde_json::Value = serde_json::from_str(&strip(text)).unwrap();
  assert_eq!(v["a"], 1);
  assert_eq!(v["b"][1], 2);
}

#[test]
fn keeps_slashes_inside_strings() {
  let text = r#"{"url": "https://example.com//a", "q": "say \"//\""}"#;
  let v: serde_json::Value = serde_json::from_str(&strip(text)).unwrap();
  assert_eq!(v["url"], "https://example.com//a");
  assert_eq!(v["q"], "say \"//\"");
}

#[test]
fn keeps_unicode() {
  let v: serde_json::Value = serde_json::from_str(&strip("{\"n\": \"héllo ✓\"}")).unwrap();
  assert_eq!(v["n"], "héllo ✓");
}
