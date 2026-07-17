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

#[test]
fn mcp_is_off_with_no_servers_by_default() {
    let s = Settings::default();
    assert!(!s.mcp.enabled);
    assert_eq!(s.mcp.expose, "direct");
    assert!(s.mcp_servers.is_empty());
}

#[test]
fn mcp_servers_deserialize_from_settings_json() {
    let json = r#"{
        "mcp": { "enabled": true, "expose": "search" },
        "mcp_servers": [
            { "name": "github", "command": "gh-mcp", "args": ["--stdio"] },
            { "name": "docs", "transport": "http", "url": "https://mcp.example.com/mcp",
              "secret": "docs_token", "enabled": false }
        ]
    }"#;
    let s: Settings = serde_json::from_str(json).unwrap();
    assert!(s.mcp.enabled);
    assert_eq!(s.mcp.expose, "search");
    assert_eq!(s.mcp_servers.len(), 2);

    let gh = &s.mcp_servers[0];
    // A stdio server omitting `transport` and `enabled` gets the ergonomic
    // defaults: stdio transport, enabled.
    assert_eq!(gh.transport, "stdio");
    assert!(gh.enabled);
    assert_eq!(gh.command, "gh-mcp");
    assert_eq!(gh.args, vec!["--stdio"]);
    assert_eq!(gh.project, 0);

    let docs = &s.mcp_servers[1];
    assert_eq!(docs.transport, "http");
    assert_eq!(docs.url, "https://mcp.example.com/mcp");
    assert_eq!(docs.secret, "docs_token");
    // An explicit `false` is honored over the default-true.
    assert!(!docs.enabled);
}
