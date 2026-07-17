use super::*;
use serde_json::json;

fn tool(name: &str) -> Value {
    json!({ "name": name, "description": format!("does {name}"), "inputSchema": { "type": "object" } })
}

#[test]
fn allow_is_a_whitelist_deny_a_blacklist() {
    // Empty allow = allow all.
    assert!(is_allowed("x", &[], &[]));
    // Allow present = only those listed.
    assert!(is_allowed("x", &["x".into()], &[]));
    assert!(!is_allowed("y", &["x".into()], &[]));
    // Deny wins even over allow.
    assert!(!is_allowed("x", &["x".into()], &["x".into()]));
    assert!(!is_allowed("z", &[], &["z".into()]));
}

#[test]
fn merge_tools_namespaces_and_preserves_the_schema() {
    let listings = vec![
        Listing::new("github", vec![tool("create_pr")]),
        Listing::new("linear", vec![tool("create_issue")]),
    ];
    let merged = merge_tools(&listings);
    let names: Vec<&str> = merged.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["github__create_pr", "linear__create_issue"]);
    // Everything but the name rides through untouched.
    assert_eq!(merged[0]["inputSchema"], json!({ "type": "object" }));
    assert_eq!(merged[0]["description"], "does create_pr");
}

#[test]
fn merge_tools_applies_the_filter() {
    let listing = Listing {
        service: "github".into(),
        allow: vec![],
        deny: vec!["dangerous".into()],
        items: vec![tool("safe"), tool("dangerous")],
    };
    let merged = merge_tools(&[listing]);
    let names: Vec<&str> = merged.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["github__safe"]);
}

#[test]
fn merge_field_namespaces_resource_uris() {
    let listings = vec![Listing::new(
        "docs",
        vec![json!({ "uri": "file:///a.md", "name": "A" })],
    )];
    let merged = merge_field(&listings, "uri");
    assert_eq!(merged[0]["uri"], "docs__file:///a.md");
    // The rest of the resource object is preserved.
    assert_eq!(merged[0]["name"], "A");
}

#[test]
fn route_reverses_a_namespaced_name() {
    assert_eq!(route("github__create_pr"), Some(("github", "create_pr")));
    assert_eq!(route("plain"), None);
}

#[test]
fn meta_tools_advertise_find_and_call() {
    let metas = meta_tools();
    let names: Vec<&str> = metas.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec![FIND_TOOL, CALL_TOOL]);
}

#[test]
fn find_matches_name_or_description_and_trims() {
    let tools = vec![
        json!({ "name": "github__create_pr", "description": "open a pull request", "inputSchema": {} }),
        json!({ "name": "linear__create_issue", "description": "file a bug", "inputSchema": {} }),
    ];
    let hits = find(&tools, "pull", 10);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["name"], "github__create_pr");
    // The trimmed result drops the schema.
    assert!(hits[0].get("inputSchema").is_none());

    // Description match works too, and is case-insensitive.
    assert_eq!(find(&tools, "BUG", 10).len(), 1);
    // Empty query returns everything up to the limit.
    assert_eq!(find(&tools, "", 1).len(), 1);
    assert_eq!(find(&tools, "", 10).len(), 2);
}
