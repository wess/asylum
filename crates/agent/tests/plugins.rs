use super::*;

#[test]
fn slugs() {
  assert_eq!(slug("Google Calendar"), "google_calendar");
  assert_eq!(slug("  GitHub! "), "github");
}

#[test]
fn schema_adds_account_choice() {
  let o = Offered {
    name: "notion__search".into(),
    plugin: "p".into(),
    tool: serde_json::from_value(json!({"name": "search", "inputSchema": {"type": "object", "properties": {"q": {"type": "string"}}}})).unwrap(),
    accounts: vec![("1".into(), "work".into()), ("2".into(), "personal".into())],
  };
  let s = schema(&o);
  assert_eq!(s["properties"]["account"]["enum"][1], "personal");
  assert_eq!(s["properties"]["q"]["type"], "string");
}
