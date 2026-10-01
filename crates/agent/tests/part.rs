use super::*;

#[test]
fn round_trips_tagged() {
  let parts = vec![
    Part::Tool { id: "c".into(), name: "shell".into(), args: serde_json::json!({"command": "ls"}), result: "a".into(), status: "done".into() },
    Part::Question { text: "Which?".into(), options: vec!["A".into()] },
  ];
  let json = serde_json::to_string(&to_values(&parts)).unwrap();
  assert!(json.contains("\"type\":\"tool\""));
  assert_eq!(parse(&json), parts);
}

#[test]
fn unknown_parts_are_skipped() {
  assert!(parse("[{\"type\":\"hologram\"}]").is_empty());
}
