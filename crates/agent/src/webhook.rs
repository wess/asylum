//! Event triggers for routines. Each routine has a URL and a key:
//! `POST http://127.0.0.1:<port>/routines/<id>` with
//! `Authorization: Bearer <key>`. The JSON body is passed to the run with the
//! instruction; `200` means a run started. App events (Slack, GitHub, Linear,
//! Sentry, PagerDuty, email relays) post to the same endpoint and are matched
//! against the routine's filter.

use crate::queue::{Job, Origin};
use crate::runtime::Runtime;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use store::routines::Routine;

pub fn url(port: u16, id: &str) -> String {
  format!("http://127.0.0.1:{port}/routines/{id}")
}

pub async fn serve(rt: Runtime) -> anyhow::Result<()> {
  let port = rt.settings().webhook_port;
  let app = Router::new().route("/routines/{id}", post(hook)).with_state(rt);
  let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
  axum::serve(listener, app).await?;
  Ok(())
}

async fn hook(State(rt): State<Runtime>, Path(id): Path<String>, headers: HeaderMap, body: Option<Json<Value>>) -> (StatusCode, Json<Value>) {
  let key = headers
    .get("authorization")
    .and_then(|v| v.to_str().ok())
    .and_then(|v| v.strip_prefix("Bearer "))
    .unwrap_or("");
  let routine = match store::routines::by_key(&rt.pool, &id, key).await {
    Ok(Some(r)) => r,
    _ => return (StatusCode::UNAUTHORIZED, Json(json!({ "error": "unknown routine or key" }))),
  };
  if !routine.active {
    return (StatusCode::CONFLICT, Json(json!({ "error": "routine is paused" })));
  }
  let body = body.map(|Json(v)| v).unwrap_or(Value::Null);
  let event = headers
    .get("x-github-event")
    .or_else(|| headers.get("x-event"))
    .and_then(|v| v.to_str().ok())
    .unwrap_or("")
    .to_string();
  if !matches(&routine, &event, &body) {
    return (StatusCode::OK, Json(json!({ "started": false, "reason": "filtered" })));
  }
  let chat = match store::chats::direct(&rt.pool, &routine.bot_id).await {
    Ok(c) => c,
    Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))),
  };
  let note = serde_json::to_string_pretty(&json!({ "event": event, "body": body })).unwrap_or_default();
  let note: String = note.chars().take(20_000).collect();
  crate::turn::enqueue(&rt, Job { bot: routine.bot_id.clone(), chat: chat.id, origin: Origin::Routine, routine: Some(routine.id.clone()), note });
  (StatusCode::OK, Json(json!({ "started": true })))
}

/// A routine's filter: `event` must equal the event name (GitHub header or
/// the body's `type`/`event`), `contains` must appear in the body text,
/// `channel` must match the body's channel, `reaction`/`mention` likewise.
pub fn matches(r: &Routine, event: &str, body: &Value) -> bool {
  let f = r.filter();
  let text = body.to_string().to_lowercase();
  let body_event = body["type"].as_str().or(body["event"]["type"].as_str()).or(body["event"].as_str()).unwrap_or("");
  if let Some(want) = f["event"].as_str().filter(|s| !s.is_empty()) {
    if !(want.eq_ignore_ascii_case(event) || want.eq_ignore_ascii_case(body_event)) {
      return false;
    }
  }
  for key in ["contains", "mention", "reaction"] {
    if let Some(want) = f[key].as_str().filter(|s| !s.is_empty()) {
      if !text.contains(&want.to_lowercase()) {
        return false;
      }
    }
  }
  if let Some(want) = f["channel"].as_str().filter(|s| !s.is_empty() && *s != "all") {
    let ch = body["channel"].as_str().or(body["event"]["channel"].as_str()).unwrap_or("");
    if !ch.trim_start_matches('#').eq_ignore_ascii_case(want.trim_start_matches('#')) {
      return false;
    }
  }
  true
}

#[cfg(test)]
#[path = "../tests/webhook.rs"]
mod tests;
