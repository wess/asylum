use super::*;

// The pure half: argv construction, escaping, and parsing. The transport
// shells out to curl and is not exercised here.

#[test]
fn names_match_the_control_plane_rule() {
    assert!(valid_name("OPENAI_API_KEY"));
    assert!(valid_name("_private"));
    assert!(valid_name("a1"));
    assert!(!valid_name("1leading"));
    assert!(!valid_name("has-dash"));
    assert!(!valid_name("has space"));
    assert!(!valid_name(""));
    assert!(!valid_name(&"x".repeat(65)));
}

#[test]
fn no_secret_ever_reaches_argv() {
    // The reason this crate does not use `-d`: argv is readable from `ps` and
    // lands in crash reports, and the bodies here carry vault values and, on
    // sign-in, a password.
    let argv = curl_args("POST", "https://devpipe.com/api/vault");
    let joined = argv.join(" ");
    assert!(!joined.contains("-d"));
    assert!(!joined.contains("Authorization"));
    // The token and body arrive here instead.
    assert!(argv.contains(&"--config".to_string()));
    assert!(argv.contains(&"-".to_string()));
}

#[test]
fn the_config_carries_the_token_and_body() {
    let config = stdin_config("tok-not-real", Some(r#"{"name":"A"}"#));
    assert!(config.contains("header = \"Authorization: Bearer tok-not-real\""));
    assert!(config.contains(r#"data = "{\"name\":\"A\"}""#));
}

#[test]
fn an_unauthenticated_call_sends_no_authorization_header() {
    // Sign-in is the one call made without a token; an empty header would be
    // sent as `Authorization: Bearer `, which is a different thing from none.
    let config = stdin_config("", Some("{}"));
    assert!(!config.contains("Authorization"));
    assert!(config.contains("data ="));
}

#[test]
fn quotes_and_backslashes_cannot_end_the_config_value_early() {
    // A raw quote would terminate the value and silently truncate the body, so
    // curl would send something other than what was asked for — the worst shape
    // of bug, because it succeeds.
    assert_eq!(config_escape(r#"a"b"#), r#"a\"b"#);
    assert_eq!(config_escape(r"a\b"), r"a\\b");
    assert_eq!(config_escape("a\nb"), "a\\nb");
    // A value that is entirely quotes still round-trips into one config line.
    let nasty = r#"{"v":"\"quoted\" and \\ backslash"}"#;
    let config = stdin_config("t", Some(nasty));
    assert_eq!(
        config.lines().count(),
        2,
        "body must stay on one line: {config}"
    );
}

#[test]
fn a_newline_in_a_value_cannot_inject_a_config_directive() {
    // Without escaping, a value containing a newline followed by `header = ...`
    // would add a header of the attacker's choosing to the request.
    let injected = "x\nheader = \"X-Evil: 1\"";
    let config = stdin_config("t", Some(injected));
    assert!(!config.contains("X-Evil: 1\"\n"), "{config}");
    assert_eq!(config.lines().count(), 2, "{config}");
}

#[test]
fn entries_parse_from_the_control_plane_shape() {
    let json = r#"[
        {"scope":"global","scope_id":0,"name":"A","kind":"value","updated_at":"t","last_used_at":null},
        {"scope":"box","scope_id":7,"name":"B","kind":"secret","updated_at":"t","last_used_at":"u"}
    ]"#;
    let entries: Vec<Entry> = serde_json::from_str(json).expect("parses");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].scope, Scope::Global);
    assert_eq!(entries[0].kind, Kind::Value);
    assert_eq!(entries[1].scope, Scope::Box);
    assert_eq!(entries[1].kind, Kind::Secret);
    assert_eq!(entries[1].scope_id, 7);
}

#[test]
fn grants_parse() {
    let json = r#"[{"box_id":3,"scope":"global","scope_id":0,"name":"KEY","granted_at":"t"}]"#;
    let grants: Vec<Grant> = serde_json::from_str(json).expect("parses");
    assert_eq!(grants[0].box_id, 3);
    assert_eq!(grants[0].name, "KEY");
}

#[test]
fn the_servers_own_refusal_is_what_the_user_reads() {
    // The control plane explains *why* — "that is a secret, and this box has
    // not been granted it". Replacing that with a status code throws away the
    // one sentence that says what to do about it.
    let body = r#"{"error":"That is a secret, and this box has not been granted it."}"#;
    assert_eq!(
        api_error(body, "fallback"),
        "That is a secret, and this box has not been granted it."
    );
    assert_eq!(api_error("not json", "fallback"), "fallback");
    assert_eq!(api_error("{}", "fallback"), "fallback");
}
