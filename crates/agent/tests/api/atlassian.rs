use super::*;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};

async fn server() -> String {
  // Accepts probe@acme.com / good-token (Basic cHJvYmVAYWNtZS5jb206Z29vZC10b2tlbg==).
  let app = Router::new().route(
    "/rest/api/2/myself",
    get(|h: HeaderMap| async move {
      let ok = h.get("authorization").and_then(|v| v.to_str().ok()) == Some("Basic cHJvYmVAYWNtZS5jb206Z29vZC10b2tlbg==");
      if ok {
        (StatusCode::OK, Json(serde_json::json!({ "displayName": "Pat Probe" })))
      } else {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({})))
      }
    }),
  );
  let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = l.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
  format!("http://{addr}")
}

fn values(url: &str, token: &str) -> HashMap<String, String> {
  [("JIRA_URL", url), ("JIRA_USERNAME", "probe@acme.com"), ("JIRA_API_TOKEN", token)].into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn checks_credentials_against_jira() {
  let url = server().await;
  assert_eq!(verify(&values(&url, "good-token")).await.unwrap(), "Pat Probe");
  let err = verify(&values(&url, "bad")).await.unwrap_err().to_string();
  assert!(err.contains("rejected"), "{err}");
  assert!(verify(&values("acme.atlassian.net", "x")).await.unwrap_err().to_string().contains("https://"));
}
