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
fn missing_file_is_clean_default() {
    let (cfg, diags) = load_project(std::path::Path::new("/no/such/dir"));
    assert_eq!(cfg, ProjectConfig::default());
    assert!(diags.is_empty());
}
