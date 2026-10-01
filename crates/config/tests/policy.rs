use super::*;

#[test]
fn missing_file_is_unmanaged() {
  let p = load(Path::new("/nonexistent/asylum/policy.json")).unwrap();
  assert!(!p.managed());
}

#[test]
fn parses_and_caps() {
  let dir = std::env::temp_dir().join(format!("asylum-policy-{}", std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();
  let f = dir.join("policy.json");
  std::fs::write(&f, r#"{
    // comments allowed
    "organization": "Acme",
    "disabled-plugins": ["stripe"],
    "local-exec": "ask",
    "templates": "team",
    "rules": [{"kind": "ask", "text": "Before emailing anyone outside acme.com"}]
  }"#).unwrap();
  let p = load(&f).unwrap();
  assert!(p.managed());
  assert_eq!(p.cap_local(LocalExec::Always), LocalExec::Ask);
  assert_eq!(p.cap_local(LocalExec::Never), LocalExec::Never);
  assert!(p.blocks(Some("stripe"), "Stripe"));
  assert!(p.blocks(None, "STRIPE"));
  assert!(!p.blocks(Some("github"), "GitHub"));
  assert!(p.allows_template("team"));
  assert!(!p.allows_template("public"));
  assert_eq!(p.rules[0].kind, "ask");
  std::fs::write(&f, "{ not json").unwrap();
  assert!(load(&f).is_err());
}

#[test]
fn network_modes() {
  let n = |mode: &str| Network { mode: mode.into(), allowed: vec!["acme.com".into(), "*.github.com".into()], blocked: vec!["facebook.com".into()] };
  assert!(n("open").allows_url("https://facebook.com/x"));
  assert!(n("").allows_url("https://anything.example"));
  assert!(!n("default").allows_url("https://www.facebook.com/"));
  assert!(n("default").allows_url("https://news.ycombinator.com"));
  assert!(n("allowlist").allows_url("https://docs.acme.com:8443/a?b"));
  assert!(n("allowlist").allows_url("https://api.github.com/repos"));
  assert!(!n("allowlist").allows_url("https://acme.com.evil.io/"));
  assert!(!n("allowlist").allows_url("https://user@evil.io/"));
  assert!(!n("offline").allows_url("https://acme.com"));
  assert!(n("offline").allows_url("about:blank"));
}
