use super::*;
use std::collections::HashMap;

fn upstream(name: &str, base: &str, secret: &str, project: i64) -> Upstream {
    Upstream {
        name: name.into(),
        base_url: base.into(),
        secret: secret.into(),
        header: String::new(),
        format: String::new(),
        project,
    }
}

fn resolver(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |name: &str| map.get(name).cloned()
}

#[test]
fn plans_a_forward_with_defaults() {
    let ups = vec![upstream("openai", "https://api.openai.com", "openai", 0)];
    let p = plan(
        "/openai/v1/chat/completions",
        &ups,
        0,
        resolver(&[("openai", "sk-live")]),
    )
    .unwrap();
    assert_eq!(p.url, "https://api.openai.com/v1/chat/completions");
    assert_eq!(p.header_name, "Authorization");
    assert_eq!(p.header_value, "Bearer sk-live");
}

#[test]
fn preserves_query_string_and_trims_base_slash() {
    let ups = vec![upstream("gh", "https://api.github.com/", "gh", 0)];
    let p = plan(
        "/gh/repos/x/y?per_page=1",
        &ups,
        0,
        resolver(&[("gh", "t")]),
    )
    .unwrap();
    assert_eq!(p.url, "https://api.github.com/repos/x/y?per_page=1");
}

#[test]
fn honors_custom_header_and_format() {
    let mut u = upstream("svc", "https://api.svc.com", "svc", 0);
    u.header = "X-Api-Key".into();
    u.format = "{secret}".into();
    let p = plan("/svc/thing", &[u], 0, resolver(&[("svc", "abc123")])).unwrap();
    assert_eq!(p.header_name, "X-Api-Key");
    assert_eq!(p.header_value, "abc123");
}

#[test]
fn project_upstream_overrides_global() {
    let ups = vec![
        upstream("api", "https://global.example.com", "api", 0),
        upstream("api", "https://project7.example.com", "api", 7),
    ];
    // Project 7 hits its own upstream; project 9 falls back to global.
    assert_eq!(
        plan("/api/x", &ups, 7, resolver(&[("api", "k")]))
            .unwrap()
            .url,
        "https://project7.example.com/x"
    );
    assert_eq!(
        plan("/api/x", &ups, 9, resolver(&[("api", "k")]))
            .unwrap()
            .url,
        "https://global.example.com/x"
    );
}

#[test]
fn unknown_upstream_and_bad_path() {
    let r = resolver(&[]);
    assert_eq!(
        plan("/nope/x", &[], 0, &r),
        Err(PlanError::UnknownUpstream("nope".into()))
    );
    assert_eq!(plan("/", &[], 0, &r), Err(PlanError::BadPath));
}

#[test]
fn missing_secret_is_an_error() {
    let ups = vec![upstream("svc", "https://api.svc.com", "svc", 0)];
    assert_eq!(
        plan("/svc/x", &ups, 0, resolver(&[])),
        Err(PlanError::MissingSecret("svc".into()))
    );
    assert_eq!(
        plan("/svc/x", &ups, 0, resolver(&[("svc", "")])),
        Err(PlanError::MissingSecret("svc".into()))
    );
}

#[test]
fn non_http_base_url_is_refused() {
    let ups = vec![upstream("bad", "file:///etc/passwd", "bad", 0)];
    assert_eq!(
        plan("/bad/x", &ups, 0, resolver(&[("bad", "k")])),
        Err(PlanError::BadBaseUrl("file:///etc/passwd".into()))
    );
}
