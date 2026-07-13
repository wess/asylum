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
