use super::*;

// The list editors write a whole model value back through `config::edit`, so the
// pure builders below are the part worth pinning: they validate the raw fields,
// preserve the fields the form does not expose, and refuse a bad entry before
// anything is written. These run without a gpui context.

#[test]
fn split_list_accepts_commas_and_whitespace() {
    assert_eq!(split_list("a, b  c\nd"), vec!["a", "b", "c", "d"]);
    assert!(split_list("   ").is_empty());
    assert_eq!(split_list(",,x,,"), vec!["x"]);
}

#[test]
fn split_ws_splits_on_whitespace_only() {
    assert_eq!(split_ws("-y  pkg\tname"), vec!["-y", "pkg", "name"]);
    assert!(split_ws("").is_empty());
}

#[test]
fn valid_slug_matches_the_gateway_rule() {
    assert!(valid_slug("github"));
    assert!(valid_slug("my-server-1"));
    assert!(!valid_slug("-bad"));
    assert!(!valid_slug("bad-"));
    assert!(!valid_slug("has_underscore"));
    assert!(!valid_slug("Upper"));
    assert!(!valid_slug(""));
}

#[test]
fn valid_secret_name_rejects_empty_and_spaces() {
    assert_eq!(
        valid_secret_name("  OPENAI_API_KEY "),
        Ok("OPENAI_API_KEY".to_string())
    );
    assert!(valid_secret_name("").is_err());
    assert!(valid_secret_name("has space").is_err());
}

#[test]
fn build_server_stdio_preserves_untouched_fields() {
    let mut base = config::McpServer::default();
    base.env.insert("FOO".to_string(), "bar".to_string());
    base.header = "X-Api".to_string();
    base.format = "Token {secret}".to_string();

    let server = build_server(
        &base,
        "github",
        "stdio",
        "npx",
        "-y server-github",
        "",
        "",
        "read_repo write",
        "delete",
        7,
        true,
    )
    .unwrap();
    assert_eq!(server.name, "github");
    assert_eq!(server.transport, "stdio");
    assert_eq!(server.command, "npx");
    assert_eq!(server.args, vec!["-y", "server-github"]);
    assert_eq!(server.allow, vec!["read_repo", "write"]);
    assert_eq!(server.deny, vec!["delete"]);
    assert_eq!(server.project, 7);
    assert!(server.enabled);
    // Fields the form does not expose survive the edit.
    assert_eq!(server.env.get("FOO"), Some(&"bar".to_string()));
    assert_eq!(server.header, "X-Api");
    assert_eq!(server.format, "Token {secret}");
}

#[test]
fn build_server_validates_name_and_transport_requirements() {
    let base = config::McpServer::default();
    // Bad namespace name.
    assert!(build_server(&base, "Bad Name", "stdio", "x", "", "", "", "", "", 0, true).is_err());
    // stdio without a command.
    assert!(build_server(&base, "svc", "stdio", "  ", "", "", "", "", "", 0, true).is_err());
    // http without a valid url.
    assert!(build_server(&base, "svc", "http", "", "", "ftp://x", "", "", "", 0, true).is_err());
    // http with a valid url is fine and drops the stdio command.
    let http = build_server(
        &base,
        "svc",
        "http",
        "",
        "",
        "https://mcp.example.com/mcp",
        "tok",
        "",
        "",
        0,
        true,
    )
    .unwrap();
    assert_eq!(http.transport, "http");
    assert_eq!(http.url, "https://mcp.example.com/mcp");
    assert_eq!(http.secret, "tok");
}

#[test]
fn build_upstream_requires_url_and_secret() {
    let base = config::Upstream::default();
    assert!(build_upstream(&base, "openai", "https://api.openai.com", "openai", 0).is_ok());
    assert!(build_upstream(&base, "openai", "api.openai.com", "openai", 0).is_err());
    assert!(build_upstream(&base, "openai", "https://api.openai.com", "  ", 0).is_err());
    assert!(build_upstream(&base, "Bad Name", "https://api.openai.com", "openai", 0).is_err());
}

#[test]
fn build_custom_agent_refuses_empty_id_or_program() {
    assert!(build_custom_agent("", "n", "", "prog", "", "arg").is_err());
    assert!(build_custom_agent("id", "n", "", "  ", "", "arg").is_err());
    assert!(build_custom_agent("has space", "n", "", "prog", "", "arg").is_err());

    let agent =
        build_custom_agent("my-agent", "", "🤖x", "run", "--prompt {prompt}", "weird").unwrap();
    // An empty name falls back to the id; the icon is a single glyph; an unknown
    // delivery mode is normalized to "arg".
    assert_eq!(agent.name, "my-agent");
    assert_eq!(agent.icon, "🤖");
    assert_eq!(agent.delivery, "arg");
    assert_eq!(agent.args, vec!["--prompt", "{prompt}"]);

    let stdin = build_custom_agent("a", "A", "", "run", "", "stdin").unwrap();
    assert_eq!(stdin.delivery, "stdin");
}

#[test]
fn build_layout_requires_name_and_agents() {
    assert!(build_layout("", "", "claude-code", 0).is_err());
    assert!(build_layout("duel", "", "   ", 0).is_err());
    let layout = build_layout("duel", "two agents", "claude-code, codex", 2).unwrap();
    assert_eq!(layout.name, "duel");
    assert_eq!(layout.agents, vec!["claude-code", "codex"]);
    assert_eq!(layout.concurrency, 2);
}

#[test]
fn scope_options_offers_global_current_and_editing_scope() {
    // No project focused: only Global.
    assert_eq!(
        scope_options(None, 0),
        vec![("0".to_string(), "Global".to_string())]
    );
    // A focused project adds "This project".
    let with_current = scope_options(Some(5), 0);
    assert_eq!(with_current.len(), 2);
    assert_eq!(with_current[1].0, "5");
    // Editing an item scoped elsewhere surfaces that scope too, without dupes.
    let elsewhere = scope_options(Some(5), 9);
    assert_eq!(elsewhere.len(), 3);
    assert_eq!(elsewhere[2], ("9".to_string(), "Project 9".to_string()));
    // The item's own scope equal to the current project is not duplicated.
    assert_eq!(scope_options(Some(5), 5).len(), 2);
}

#[test]
fn scope_key_helpers_round_trip() {
    assert_eq!(scope_display("global"), "Global keep");
    assert_eq!(scope_display("project:42"), "Project 42 keep");
    assert_eq!(scope_key_project("global"), 0);
    assert_eq!(scope_key_project("project:42"), 42);
}
