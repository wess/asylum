use super::*;

#[test]
fn parses_teams() {
    let json = r#"{"data":{"teams":{"nodes":[
        {"id":"t1","key":"ENG","name":"Engineering"},
        {"id":"t2","key":"DES","name":"Design"}
    ]}}}"#;
    let teams = parse_teams(json).unwrap();
    assert_eq!(teams.len(), 2);
    assert_eq!(teams[0].key, "ENG");
}

#[test]
fn parses_issues_with_state() {
    let json = r#"{"data":{"issues":{"nodes":[
        {"id":"i1","identifier":"ENG-12","title":"Fix login","url":"https://l/ENG-12",
         "priority":2,"state":{"name":"In Progress"}}
    ]}}}"#;
    let issues = parse_issues(json).unwrap();
    assert_eq!(issues[0].identifier, "ENG-12");
    assert_eq!(issues[0].state, "In Progress");
    assert_eq!(issues[0].priority, 2.0);
}

#[test]
fn parses_projects() {
    let json = r#"{"data":{"projects":{"nodes":[{"id":"p1","name":"Q3","state":"started"}]}}}"#;
    let ps = parse_projects(json).unwrap();
    assert_eq!(ps[0].name, "Q3");
    assert_eq!(ps[0].state, "started");
}

#[test]
fn surfaces_graphql_errors() {
    let json = r#"{"errors":[{"message":"unauthorized"}]}"#;
    assert!(matches!(parse_teams(json), Err(Error::Api(_))));
}

#[test]
fn missing_nodes_is_parse_error() {
    let json = r#"{"data":{}}"#;
    assert!(matches!(parse_teams(json), Err(Error::Parse(_))));
}

#[test]
fn client_endpoint_override() {
    let c = Client::new("lin_key").with_endpoint("http://localhost:9999/graphql");
    // No network call here - just confirm the builder is wired.
    let _ = c;
}

#[test]
fn token_is_kept_off_the_argv() {
    // The command line must never carry the token; it goes on stdin instead.
    let args = curl_args("https://api.linear.app/graphql", r#"{"query":"x"}"#);
    assert!(
        !args.iter().any(|a| a.contains("lin_api_secret")),
        "token leaked into argv: {args:?}"
    );
    assert!(args.iter().any(|a| a == "--config"));
    // The auth header is delivered via the stdin config instead.
    let cfg = auth_config("lin_api_secret");
    assert!(cfg.contains("Authorization: lin_api_secret"));
    assert!(cfg.starts_with("header = "));
}

#[test]
fn errors_redact_the_token() {
    let leaked = "curl failed using key lin_api_secret over the wire";
    assert_eq!(
        redact(leaked, "lin_api_secret"),
        "curl failed using key *** over the wire"
    );
    // A blank key is a no-op.
    assert_eq!(redact("nothing to hide", ""), "nothing to hide");
}

#[test]
fn non_http_endpoints_are_refused() {
    let c = Client::new("k").with_endpoint("file:///etc/passwd");
    assert!(matches!(
        c.query("q", serde_json::Value::Null),
        Err(Error::Api(_))
    ));
    assert!(is_http_url("https://api.linear.app/graphql"));
    assert!(is_http_url("http://localhost:9999"));
    assert!(!is_http_url("ftp://x"));
}
