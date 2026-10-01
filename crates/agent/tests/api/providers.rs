use super::*;

#[test]
fn pasted_keys_live_in_the_keychain() {
  let p = keyed_profile("lite-llm", "work", "https://gw.acme.dev/v1/");
  assert_eq!(p.endpoint, "https://gw.acme.dev/v1");
  assert_eq!(p.credential, Credential::Keychain { account: provider::credential::account("work") });
  // xAI keeps its own entry, shared with voice and images.
  let x = keyed_profile("xai", "xai", "");
  assert_eq!(x.credential, Credential::Keychain { account: config::secret::XAI_KEY.into() });
  assert!(needs_endpoint("lite-llm") && needs_endpoint("custom") && !needs_endpoint("openai"));
}

/// Paste a LiteLLM key: a refused key isn't saved; an accepted one lands in
/// the Keychain, the profile is added with its models, and it's the default.
#[tokio::test(flavor = "multi_thread")]
async fn connects_litellm_with_a_pasted_key() {
  use axum::http::{HeaderMap, StatusCode};
  use axum::routing::get;
  use axum::{Json, Router};
  use serde_json::json;
  let app = Router::new().route("/v1/models", get(|h: HeaderMap| async move {
    if h.get("authorization").and_then(|v| v.to_str().ok()) != Some("Bearer sk-pasted") {
      return (StatusCode::UNAUTHORIZED, Json(json!({"error": {"message": "Invalid key"}})));
    }
    (StatusCode::OK, Json(json!({"data": [{"id": "claude-sonnet"}, {"id": "gpt-4o"}]})))
  }));
  let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = l.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(l, app).await.unwrap() });

  let dir = tempfile::tempdir().unwrap();
  // Keep the test's settings file out of the real config folder.
  std::env::set_var("XDG_CONFIG_HOME", dir.path());
  let rt = Runtime::start(dir.path().join("data"), config::Settings { memory: false, ..Default::default() }).await.unwrap();
  let name = format!("gwtest{}", std::process::id());
  let url = format!("http://{addr}/v1");
  let err = connect(&rt, "lite-llm", &name, &url, "sk-wrong").await.unwrap_err().to_string();
  assert!(err.contains("refused this key"), "{err}");
  assert!(config::secret::get(&provider::credential::account(&name)).is_none());

  let p = connect(&rt, "lite-llm", &name, &url, "  sk-pasted ").await.unwrap();
  assert_eq!(p.models, ["claude-sonnet", "gpt-4o"]);
  assert_eq!(provider::credential::resolve_for(&p).await.unwrap().as_deref(), Some("sk-pasted"));
  let s = rt.settings();
  assert_eq!((s.provider.as_str(), s.model.as_str()), (name.as_str(), "claude-sonnet"));
  let _ = config::secret::remove(&provider::credential::account(&name));
}
