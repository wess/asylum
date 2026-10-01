use super::*;

#[test]
fn manifest_has_socket_mode_and_events() {
  let m = build("Support Agent", "Answers support questions");
  assert_eq!(m["settings"]["socket_mode_enabled"], true);
  assert!(m["oauth_config"]["scopes"]["bot"].as_array().unwrap().iter().any(|s| s == "chat:write"));
  assert!(m["settings"]["event_subscriptions"]["bot_events"].as_array().unwrap().iter().any(|e| e == "app_mention"));
  assert!(create_url(&m).starts_with("https://api.slack.com/apps?new_app=1&manifest_json=%7B"));
}
