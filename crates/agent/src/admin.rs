//! The Admin API: a local HTTP API on `127.0.0.1:<admin_port>` for scripts
//! and dashboards. Every request needs `Authorization: Bearer <token>`; the
//! token is generated when the API is turned on and kept in the keychain.
//!
//! GET  /v1/agents                     Agents and their status
//! GET  /v1/actions?since=<ms>       Action Recording
//! GET  /v1/audit?since=<ms>         Audit log
//! GET  /v1/insights?since=<ms>      Conversation Insights
//! GET  /v1/chats/{id}/export        Conversation content (JSON)
//! POST /v1/chats/{id}/messages      {"text": "..."} sends as the user

use crate::runtime::Runtime;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

pub const TOKEN: &str = "admin-api-token";

type Reply = (StatusCode, Json<Value>);

#[derive(Deserialize)]
pub struct Since {
  #[serde(default)]
  since: i64,
}

/// The API token, created on first use.
pub fn token() -> anyhow::Result<String> {
  if let Some(t) = config::secret::get(TOKEN) {
    return Ok(t);
  }
  let t = format!("asy_{}", uuid::Uuid::new_v4().simple());
  config::secret::set(TOKEN, &t)?;
  Ok(t)
}

pub fn router(rt: Runtime, token: String) -> Router {
  Router::new()
    .route("/v1/agents", get(agents))
    .route("/v1/actions", get(actions))
    .route("/v1/audit", get(audit))
    .route("/v1/insights", get(insights))
    .route("/v1/chats/{id}/export", get(export))
    .route("/v1/chats/{id}/messages", post(message))
    .layer(axum::middleware::from_fn(move |req: axum::extract::Request, next: axum::middleware::Next| {
      let ok = authorized(req.headers(), &token);
      async move {
        if ok {
          next.run(req).await
        } else {
          axum::response::IntoResponse::into_response((StatusCode::UNAUTHORIZED, Json(json!({ "error": "bad or missing token" }))))
        }
      }
    }))
    .with_state(rt)
}

fn authorized(h: &HeaderMap, token: &str) -> bool {
  let got = h.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("");
  // Constant-time compare.
  got.len() == token.len() && got.bytes().zip(token.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

/// Serve until the process ends, if the port is set.
pub async fn serve(rt: Runtime) -> anyhow::Result<()> {
  let port = rt.settings().admin_port;
  if port == 0 {
    return Ok(());
  }
  let app = router(rt, token()?);
  let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
  axum::serve(listener, app).await?;
  Ok(())
}

fn ok(v: impl serde::Serialize) -> Reply {
  (StatusCode::OK, Json(serde_json::to_value(v).unwrap_or(Value::Null)))
}

fn fail(e: anyhow::Error) -> Reply {
  (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() })))
}

async fn agents(State(rt): State<Runtime>) -> Reply {
  match store::bots::list(&rt.pool).await {
    Ok(b) => ok(b.into_iter().map(|b| json!({ "id": b.id, "name": b.name, "status": b.status, "kind": b.kind, "model": b.model })).collect::<Vec<_>>()),
    Err(e) => fail(e),
  }
}

async fn actions(State(rt): State<Runtime>, Query(q): Query<Since>) -> Reply {
  store::events::list(&rt.pool, store::events::ACTION, q.since, 10_000).await.map(ok).unwrap_or_else(fail)
}

async fn audit(State(rt): State<Runtime>, Query(q): Query<Since>) -> Reply {
  store::events::list(&rt.pool, store::events::AUDIT, q.since, 10_000).await.map(ok).unwrap_or_else(fail)
}

async fn insights(State(rt): State<Runtime>, Query(q): Query<Since>) -> Reply {
  crate::api::insights::since(&rt, q.since).await.map(ok).unwrap_or_else(fail)
}

async fn export(State(rt): State<Runtime>, Path(id): Path<String>) -> Reply {
  crate::api::export::chat_json(&rt, &id).await.map(ok).unwrap_or_else(fail)
}

async fn message(State(rt): State<Runtime>, Path(id): Path<String>, Json(body): Json<Value>) -> Reply {
  let text = body["text"].as_str().unwrap_or("").trim().to_string();
  if text.is_empty() {
    return fail(anyhow::anyhow!("text is required"));
  }
  crate::api::chat::send(&rt, &id, &text, &[], None).await.map(|m| ok(json!({ "id": m.id }))).unwrap_or_else(fail)
}

#[cfg(test)]
#[path = "../tests/admin.rs"]
mod tests;
