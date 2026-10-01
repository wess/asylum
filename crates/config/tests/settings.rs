use super::*;

#[test]
fn empty_is_defaults() {
  let l = parse("");
  assert_eq!(l.settings, Settings::default());
  assert!(l.diagnostics.is_empty());
}

#[test]
fn bad_value_keeps_default_and_reports() {
  let l = parse("{ \"max-steps\": \"lots\", \"model\": \"grok-x\" }");
  assert_eq!(l.settings.max_steps, Settings::default().max_steps);
  assert_eq!(l.settings.model, "grok-x");
  assert_eq!(l.diagnostics.len(), 1);
}

#[test]
fn unknown_key_is_reported() {
  let l = parse("{ \"nope\": 1 }");
  assert!(l.diagnostics[0].contains("nope"));
}

#[test]
fn save_writes_only_changes_and_round_trips() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("settings.json");
  let s = Settings {
    appearance: Appearance::Dark,
    ..Default::default()
  };
  save(&path, &s).unwrap();
  let text = std::fs::read_to_string(&path).unwrap();
  assert!(text.contains("\"appearance\": \"dark\""));
  assert!(!text.contains("model"));
  assert_eq!(load(&path).settings, s);
}
