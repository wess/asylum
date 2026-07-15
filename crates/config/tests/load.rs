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

#[test]
fn env_fills_only_empty_secrets() {
    // Empty secrets are filled from the environment override.
    let mut s = Settings::default();
    resolve_secrets(&mut s, Some("lin_tok".into()), Some("comp_tok".into()));
    assert_eq!(s.linear_token, "lin_tok");
    assert_eq!(s.companion.token, "comp_tok");

    // A value already in the file wins over the environment.
    let mut s = Settings {
        linear_token: "from_file".into(),
        ..Default::default()
    };
    resolve_secrets(&mut s, Some("from_env".into()), None);
    assert_eq!(s.linear_token, "from_file");

    // A blank override is ignored.
    let mut s = Settings::default();
    resolve_secrets(&mut s, Some("   ".into()), None);
    assert!(s.linear_token.is_empty());
}
