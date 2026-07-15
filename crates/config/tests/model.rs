use super::*;

#[test]
fn builtin_layouts_ship_by_default() {
    let s = Settings::default();
    assert!(s.layout("duel").is_some());
    let triad = s.layout("triad").expect("triad preset");
    assert_eq!(triad.agents.len(), 3);
    // Lookup is case-insensitive.
    assert!(s.layout("SWARM").is_some());
    assert!(s.layout("nope").is_none());
}

#[test]
fn layouts_deserialize_from_settings_json() {
    let json = r#"{
        "layouts": [
            { "name": "solo", "description": "just one", "agents": ["claude-code"] },
            { "name": "pair", "agents": ["claude-code", "codex"], "concurrency": 2 }
        ]
    }"#;
    let s: Settings = serde_json::from_str(json).unwrap();
    // A user-provided list replaces the built-ins entirely.
    assert_eq!(s.layouts.len(), 2);
    let pair = s.layout("pair").unwrap();
    assert_eq!(pair.concurrency, 2);
    assert_eq!(pair.agents, vec!["claude-code", "codex"]);
    // Absent description defaults to empty; absent concurrency to 0.
    assert_eq!(pair.description, "");
    assert_eq!(s.layout("solo").unwrap().concurrency, 0);
}

#[test]
fn a_default_settings_has_no_layout_named_garbage() {
    assert!(Settings::default().layout("").is_none());
}
