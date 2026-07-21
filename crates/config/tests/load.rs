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
fn enabled_plugins_survive_edit_and_load() {
    // Writing the enabled list through the comment-preserving editor and
    // reloading round-trips the value; removing the key restores the default.
    let base = "// my settings\n{\n    \"theme\": \"nord\"\n}\n";
    let with = crate::edit::upsert(base, "enabled_plugins", "[\"acme.hello\", \"beta\"]").unwrap();
    let loaded = load_str(&with);
    assert!(loaded.diagnostics.is_empty());
    assert_eq!(loaded.settings.enabled_plugins, vec!["acme.hello", "beta"]);
    // The user's comment and other keys are untouched by the write.
    assert!(with.contains("// my settings"));
    assert_eq!(loaded.settings.theme, "nord");

    let without = crate::edit::remove(&with, "enabled_plugins").unwrap();
    assert!(load_str(&without).settings.enabled_plugins.is_empty());
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

/// A typo must cost only the key it is on. `deny_unknown_fields` rejects the
/// whole document, so without per-key salvage one bad line would silently
/// revert every other setting the user had.
#[test]
fn a_bad_key_does_not_discard_the_good_ones() {
    let src = r#"{
        "theme": "nord",
        "max_parallel_runs": 8,
        "theem": "typo",
        "editor": { "font_size": 20.0 }
    }"#;
    let loaded = load_str(src);

    // The good keys survive.
    assert_eq!(loaded.settings.theme, "nord");
    assert_eq!(loaded.settings.max_parallel_runs, 8);
    assert_eq!(loaded.settings.editor.font_size, 20.0);
    // And the bad one is reported by name.
    assert_eq!(loaded.diagnostics.len(), 1);
    assert_eq!(loaded.diagnostics[0].key, "theem");
}

/// A well-named key holding the wrong type is reported too, and likewise costs
/// only itself.
#[test]
fn a_wrongly_typed_key_does_not_discard_the_good_ones() {
    let loaded = load_str(r#"{ "theme": "nord", "max_parallel_runs": "lots" }"#);
    assert_eq!(loaded.settings.theme, "nord");
    // The bad key falls back to its default rather than taking `theme` with it.
    assert_eq!(
        loaded.settings.max_parallel_runs,
        Settings::default().max_parallel_runs
    );
    assert_eq!(loaded.diagnostics.len(), 1);
    assert_eq!(loaded.diagnostics[0].key, "max_parallel_runs");
}

/// Salvage applies to nested structures too: a bad key inside `companion` must
/// not cost the user their top-level settings.
#[test]
fn a_bad_nested_key_does_not_discard_the_top_level() {
    let loaded = load_str(r#"{ "theme": "nord", "companion": { "bogus": 1 } }"#);
    assert_eq!(loaded.settings.theme, "nord");
    assert_eq!(loaded.diagnostics.len(), 1);
    assert_eq!(loaded.diagnostics[0].key, "companion");
}

/// Malformed JSON has no salvageable structure - defaults, one diagnostic, no
/// panic.
#[test]
fn malformed_json_falls_back_cleanly() {
    for src in [
        r#"{ "theme": "nord",, }"#, // stray comma
        r#"{ "theme": "nord" "#,    // unterminated object
        r#"{ theme: "nord" }"#,     // unquoted key
    ] {
        let loaded = load_str(src);
        assert_eq!(loaded.settings, Settings::default(), "src: {src}");
        assert_eq!(loaded.diagnostics.len(), 1, "src: {src}");
    }
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

/// `validate` (see `validate.rs`) runs at the tail of `load_str`, so a
/// type-valid but semantically bad value - unlike a typo'd key - keeps the
/// rest of the document and still earns a diagnostic naming the key.
#[test]
fn semantically_bad_value_surfaces_as_a_diagnostic() {
    let loaded = load_str(r#"{ "theme": "nord", "worktree_dir": "" }"#);
    assert_eq!(loaded.settings.theme, "nord");
    assert_eq!(
        loaded.settings.worktree_dir,
        Settings::default().worktree_dir
    );
    assert_eq!(loaded.diagnostics.len(), 1);
    assert_eq!(loaded.diagnostics[0].key, "worktree_dir");
}

/// Two type-valid keys that are individually fine but jointly nonsensical
/// (colliding server ports) are still caught once the document deserializes.
#[test]
fn cross_key_semantic_problem_surfaces_as_a_diagnostic() {
    let loaded = load_str(r#"{ "control": { "bind": "127.0.0.1:8787" } }"#);
    assert_eq!(loaded.diagnostics.len(), 2);
    let keys: Vec<&str> = loaded.diagnostics.iter().map(|d| d.key.as_str()).collect();
    assert!(keys.contains(&"companion.bind"), "{keys:?}");
    assert!(keys.contains(&"control.bind"), "{keys:?}");
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
