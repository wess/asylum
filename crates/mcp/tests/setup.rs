use super::*;
use config::McpServer;

#[test]
fn secret_ref_parses_the_reference_form() {
    assert_eq!(secret_ref("{secret:OPENAI_KEY}"), Some("OPENAI_KEY"));
    assert_eq!(secret_ref("  {secret:X}  "), Some("X"));
    // A literal value is not a reference.
    assert_eq!(secret_ref("plain"), None);
    assert_eq!(secret_ref("{secret:}"), None);
    assert_eq!(secret_ref("{other:X}"), None);
}

#[test]
fn format_auth_applies_defaults() {
    // Empty header/format fall back to Authorization + Bearer, like the proxy.
    let (name, value) = format_auth("", "", "tok");
    assert_eq!(name, "Authorization");
    assert_eq!(value, "Bearer tok");
    // Explicit header/format are honored, with {secret} substituted.
    let (name, value) = format_auth("X-Api-Key", "{secret}", "abc");
    assert_eq!(name, "X-Api-Key");
    assert_eq!(value, "abc");
}

#[test]
fn a_bad_namespace_is_skipped_with_a_warning() {
    let servers = vec![McpServer {
        name: "Bad Name".into(),
        command: "true".into(),
        ..Default::default()
    }];
    let (host, warnings) = connect(&servers, |_, _| None);
    assert!(host.is_empty());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("Bad Name"));
}

#[test]
fn a_disabled_server_is_ignored() {
    let servers = vec![McpServer {
        name: "github".into(),
        command: "true".into(),
        enabled: false,
        ..Default::default()
    }];
    let (host, warnings) = connect(&servers, |_, _| None);
    assert!(host.is_empty());
    assert!(warnings.is_empty());
}

#[test]
fn an_http_server_without_a_url_warns() {
    let servers = vec![McpServer {
        name: "docs".into(),
        transport: "http".into(),
        ..Default::default()
    }];
    let (host, warnings) = connect(&servers, |_, _| None);
    assert!(host.is_empty());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("no url"));
}
