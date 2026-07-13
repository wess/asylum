use super::*;
use crate::model::Settings;

#[test]
fn empty_yields_defaults() {
    let loaded = load_str("");
    assert_eq!(loaded.settings, Settings::default());
    assert!(loaded.diagnostics.is_empty());
}

#[test]
fn partial_overrides_defaults() {
    let loaded = load_str(r#"{ "theme": "solarized", "max_parallel_runs": 8 }"#);
    assert!(loaded.diagnostics.is_empty());
    assert_eq!(loaded.settings.theme, "solarized");
    assert_eq!(loaded.settings.max_parallel_runs, 8);
    // Untouched fields keep their defaults.
    assert_eq!(loaded.settings.editor.font_size, 13.0);
}

#[test]
fn comments_are_allowed() {
    let src = r#"{
        // pick a theme
        "theme": "nord",
        "default_agents": ["claude-code", "codex"] /* fan-out */
    }"#;
    let loaded = load_str(src);
    assert!(loaded.diagnostics.is_empty());
    assert_eq!(loaded.settings.theme, "nord");
    assert_eq!(loaded.settings.default_agents.len(), 2);
}

#[test]
fn unknown_field_is_a_diagnostic_not_a_crash() {
    let loaded = load_str(r#"{ "nonsense": true }"#);
    assert_eq!(loaded.settings, Settings::default());
    assert_eq!(loaded.diagnostics.len(), 1);
    assert_eq!(loaded.diagnostics[0].key, "nonsense");
}

#[test]
fn nested_agent_prefs() {
    let src = r#"{ "agents": { "codex": { "extra_args": ["--fast"], "enabled": false } } }"#;
    let loaded = load_str(src);
    assert!(loaded.diagnostics.is_empty());
    let codex = &loaded.settings.agents["codex"];
    assert_eq!(codex.extra_args, vec!["--fast"]);
    assert_eq!(codex.enabled, Some(false));
}

#[test]
fn missing_file_is_clean_defaults() {
    let loaded = load(std::path::Path::new("/nonexistent/asylum/settings.json"));
    assert!(loaded.diagnostics.is_empty());
    assert_eq!(loaded.settings, Settings::default());
}
