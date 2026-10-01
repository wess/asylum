use super::*;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

fn authed(h: &HeaderMap) -> bool {
  h.get("authorization").and_then(|v| v.to_str().ok()) == Some("Bearer sk-good")
}

/// A gateway where `/team/available` doesn't exist (older LiteLLM), the
/// team list is at `/team/list`, and the key is narrower than its team.
async fn gateway() -> String {
  let app = Router::new()
    .route("/v1/models", get(|h: HeaderMap| async move {
      if !authed(&h) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": {"message": "Invalid proxy server token passed"}})));
      }
      (StatusCode::OK, Json(json!({"data": [{"id": "gpt-4o"}, {"id": "claude-sonnet"}, {"id": "claude-opus"}, {"id": "gpt-4o"}]})))
    }))
    .route("/team/list", get(|h: HeaderMap| async move {
      if !authed(&h) { return (StatusCode::UNAUTHORIZED, Json(json!({}))); }
      (StatusCode::OK, Json(json!([
        {"team_id": "t-eng", "team_alias": "Engineering", "models": ["claude-sonnet", "claude-opus", "all-proxy-models"]},
        {"team_id": "t-all", "models": []},
        {"team_alias": "no id, skipped"}
      ])))
    }))
    .route("/key/info", get(|h: HeaderMap| async move {
      if !authed(&h) { return (StatusCode::UNAUTHORIZED, Json(json!({}))); }
      (StatusCode::OK, Json(json!({"key": "sk-…", "info": {"models": ["claude-sonnet"]}})))
    }));
  let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = l.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
  format!("http://{addr}/v1")
}

#[test]
fn error_shapes() {
  assert_eq!(reason(r#"{"detail":{"error":"DB not connected."}}"#), "DB not connected.");
  assert_eq!(reason(r#"{"error":{"message":"No connected db.","type":"no_db_connection"}}"#), "No connected db.");
  assert_eq!(reason(r#"{"detail":"Not Found"}"#), "Not Found");
}

/// A LiteLLM running without its database (as many local proxies do):
/// teams aren't an error, just absent.
#[tokio::test(flavor = "multi_thread")]
async fn gateway_without_teams() {
  let app = Router::new()
    .route("/team/available", get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": {"error": "DB not connected."}}))) }))
    .route("/team/list", get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"detail": {"error": "DB not connected."}}))) }))
    .route("/refused/team/available", get(|| async { (StatusCode::UNAUTHORIZED, Json(json!({"error": {"message": "Invalid key"}}))) }));
  let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = l.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
  assert!(teams(&format!("http://{addr}/v1"), "k").await.unwrap().is_empty());
  let err = teams(&format!("http://{addr}/refused/v1"), "k").await.unwrap_err().to_string();
  assert!(err.contains("refused this key"), "{err}");
}

#[test]
fn roots_and_shapes() {
  assert_eq!(root("https://gw.acme.dev/v1/"), "https://gw.acme.dev");
  assert_eq!(root("http://127.0.0.1:4000"), "http://127.0.0.1:4000");
  let wrapped = parse_teams(&json!({"teams": [{"team_id": "a", "team_alias": "A", "models": []}]}));
  assert_eq!(wrapped, vec![Team { id: "a".into(), name: "A".into(), models: vec![] }]);
  assert_eq!(parse_teams(&json!({"data": [{"team_id": "b"}]}))[0].name, "b");
}

#[tokio::test(flavor = "multi_thread")]
async fn key_teams_models_and_reach() {
  let gw = gateway().await;
  let err = check(&gw, "sk-bad").await.unwrap_err().to_string();
  assert!(err.contains("refused this key") && err.contains("Invalid proxy server token"), "{err}");
  check(&gw, "sk-good").await.unwrap();

  let found = teams(&gw, "sk-good").await.unwrap();
  assert_eq!(found.len(), 2);
  let eng = &found[0];
  assert_eq!((eng.id.as_str(), eng.name.as_str()), ("t-eng", "Engineering"));
  assert_eq!(eng.models, ["claude-sonnet", "claude-opus"]);

  // Team-scoped list, the whole catalog for an unscoped team, deduped and sorted.
  assert_eq!(models(&gw, "sk-good", Some(eng)).await.unwrap(), ["claude-opus", "claude-sonnet"]);
  assert_eq!(models(&gw, "sk-good", Some(&found[1])).await.unwrap(), ["claude-opus", "claude-sonnet", "gpt-4o"]);

  let r = reach(&gw, "sk-good", Some(eng)).await;
  assert_eq!(r.missing(), ["claude-opus"]);
}

/// Fallback: a provider that fails before answering hands over to the next;
/// one that has started answering is never replaced.
#[tokio::test(flavor = "multi_thread")]
async fn fallback_moves_on_only_before_the_first_event() {
  use futures::StreamExt;
  let app = Router::new()
    .route("/down/v1/chat/completions", post(|| async { (StatusCode::BAD_GATEWAY, "upstream down") }))
    .route("/up/v1/chat/completions", post(|| async {
      let body = "data: {\"choices\":[{\"delta\":{\"content\":\"from backup\"}}]}\n\ndata: [DONE]\n\n";
      ([("content-type", "text/event-stream")], body)
    }));
  let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = l.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
  let http = |path: &str| crate::Provider::Http { client: chat::Client::new("k", Some(format!("http://{addr}/{path}/v1"))).retries(0), model: "m".into(), reasoning: None };
  let p = http("down").with_fallbacks(vec![http("up")]);
  assert!(matches!(p, crate::Provider::Fallback(ref c) if c.len() == 2));
  let mut s = p.stream(vec![chat::Message::user("hi")], vec![]).await.unwrap();
  let mut text = String::new();
  while let Some(ev) = s.next().await {
    if let chat::StreamEvent::Text(t) = ev.unwrap() {
      text.push_str(&t);
    }
  }
  assert_eq!(text, "from backup");
  // Nothing to fall back to: the provider's own error comes through.
  let alone = http("down").with_fallbacks(vec![]);
  assert!(matches!(alone, crate::Provider::Http { .. }));
}
