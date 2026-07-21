use super::*;

#[test]
fn empty_is_default() {
    let (cfg, diags) = parse_project("");
    assert_eq!(cfg, ProjectConfig::default());
    assert!(diags.is_empty());
}

#[test]
fn parses_full_project() {
    let text = r#"
base_branch = "develop"
default_agents = ["claude-code", "codex"]
setup = ["bun install", "bun run build"]

[env]
NODE_ENV = "test"
"#;
    let (cfg, diags) = parse_project(text);
    assert!(diags.is_empty());
    assert_eq!(cfg.base_branch.as_deref(), Some("develop"));
    assert_eq!(cfg.default_agents, vec!["claude-code", "codex"]);
    assert_eq!(cfg.setup.len(), 2);
    assert_eq!(cfg.env.get("NODE_ENV").map(String::as_str), Some("test"));
}

#[test]
fn unknown_key_is_diagnostic() {
    let (cfg, diags) = parse_project("nonsense = true\n");
    assert_eq!(cfg, ProjectConfig::default());
    assert_eq!(diags.len(), 1);
}

#[test]
fn repo_config_cannot_set_credentials_or_binds() {
    // A committed asylum.toml must not be able to introduce secrets or server
    // binds: those keys are unknown to ProjectConfig (deny_unknown_fields), so
    // the file is rejected to defaults rather than silently applying them.
    for hostile in [
        "linear_token = \"lin_secret\"\n",
        "[companion]\ntoken = \"x\"\nbind = \"0.0.0.0:8787\"\n",
        "[control]\ntoken = \"x\"\n",
    ] {
        let (cfg, diags) = parse_project(hostile);
        assert_eq!(
            cfg,
            ProjectConfig::default(),
            "hostile config applied: {hostile}"
        );
        assert!(!diags.is_empty(), "no diagnostic for: {hostile}");
    }
}

/// `validate_project` (see `validate.rs`) runs right after a clean TOML
/// parse, so a type-valid but nonsensical `base_branch` - unlike a bad key -
/// still keeps the rest of the document and is cleared with a diagnostic
/// naming the key, rather than rejecting the whole file.
#[test]
fn semantically_bad_base_branch_is_cleared_not_rejected() {
    let (cfg, diags) = parse_project("base_branch = \"bad..branch\"\nsetup = [\"bun install\"]\n");
    assert_eq!(cfg.base_branch, None);
    assert_eq!(cfg.setup, vec!["bun install"]);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].key, "base_branch");
}

#[test]
fn missing_file_is_clean_default() {
    let (cfg, diags) = load_project(std::path::Path::new("/no/such/dir"));
    assert_eq!(cfg, ProjectConfig::default());
    assert!(diags.is_empty());
}
