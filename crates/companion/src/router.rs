//! Request routing - pure over a [`store::Db`], so it is tested without sockets.

use serde_json::json;
use store::Db;

/// A ready-to-write HTTP response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub content_type: String,
    pub body: String,
}

impl Response {
    pub fn json(status: u16, value: serde_json::Value) -> Self {
        Response {
            status,
            content_type: "application/json".into(),
            body: value.to_string(),
        }
    }
    pub fn text(status: u16, body: &str) -> Self {
        Response {
            status,
            content_type: "text/plain; charset=utf-8".into(),
            body: body.to_string(),
        }
    }
    pub fn html(body: &str) -> Self {
        Response {
            status: 200,
            content_type: "text/html; charset=utf-8".into(),
            body: body.to_string(),
        }
    }
}

/// Dispatch `(method, path, body)` against the store.
pub fn route(method: &str, path: &str, body: &str, db: &Db) -> Response {
    // Strip a query string.
    let path = path.split('?').next().unwrap_or(path);
    match (method, path) {
        ("GET", "/") => Response::html(MOBILE_HTML),
        ("GET", "/api/health") => Response::json(200, json!({ "ok": true })),
        ("GET", "/api/projects") => projects(db),
        ("GET", "/api/notifications") => notifications(db),
        ("GET", p) if p.starts_with("/api/projects/") && p.ends_with("/tasks") => {
            match parse_id(p, "/api/projects/", "/tasks") {
                Some(id) => tasks(db, id),
                None => Response::text(404, "not found"),
            }
        }
        ("GET", p) if p.starts_with("/api/tasks/") && p.ends_with("/runs") => {
            match parse_id(p, "/api/tasks/", "/runs") {
                Some(id) => runs(db, id),
                None => Response::text(404, "not found"),
            }
        }
        ("POST", p) if p.starts_with("/api/tasks/") && p.ends_with("/followup") => {
            match parse_id(p, "/api/tasks/", "/followup") {
                Some(id) => followup(db, id, body),
                None => Response::text(404, "not found"),
            }
        }
        _ => Response::text(404, "not found"),
    }
}

fn parse_id(path: &str, prefix: &str, suffix: &str) -> Option<i64> {
    path.strip_prefix(prefix)?.strip_suffix(suffix)?.parse().ok()
}

fn projects(db: &Db) -> Response {
    let list: Vec<_> = db
        .projects()
        .unwrap_or_default()
        .into_iter()
        .map(|p| json!({ "id": p.id, "name": p.name, "pinned": p.pinned }))
        .collect();
    Response::json(200, json!(list))
}

fn tasks(db: &Db, project_id: i64) -> Response {
    let list: Vec<_> = db
        .tasks(project_id)
        .unwrap_or_default()
        .into_iter()
        .map(|t| json!({ "id": t.id, "title": t.title, "status": t.status.as_str() }))
        .collect();
    Response::json(200, json!(list))
}

fn runs(db: &Db, task_id: i64) -> Response {
    let list: Vec<_> = db
        .runs(task_id)
        .unwrap_or_default()
        .into_iter()
        .map(|r| json!({ "id": r.id, "agent": r.agent, "branch": r.branch, "status": r.status.as_str() }))
        .collect();
    Response::json(200, json!(list))
}

fn notifications(db: &Db) -> Response {
    let list: Vec<_> = db
        .notifications(false)
        .unwrap_or_default()
        .into_iter()
        .map(|n| json!({ "id": n.id, "kind": n.kind, "title": n.title, "body": n.body, "read": n.read }))
        .collect();
    Response::json(200, json!({ "unread": db.unread_count().unwrap_or(0), "items": list }))
}

/// Record a follow-up from the phone: a notification the desktop app surfaces
/// and appends to the task. Stored as a `followup` notification carrying the
/// message and task id.
fn followup(db: &Db, task_id: i64, body: &str) -> Response {
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(String::from))
        .unwrap_or_else(|| body.trim().to_string());
    if message.is_empty() {
        return Response::text(400, "empty follow-up");
    }
    let Ok(task) = db.task(task_id) else {
        return Response::text(404, "no such task");
    };
    let _ = db.notify("followup", &task.title, &message, None, now());
    Response::json(200, json!({ "ok": true, "task": task_id }))
}

/// Unix seconds. The companion runs on real wall-clock time.
fn now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The mobile status page. Self-contained; polls the JSON API.
pub const MOBILE_HTML: &str = r#"<!doctype html>
<html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Asylum</title>
<style>
  body { font: 15px -apple-system, system-ui, sans-serif; margin: 0; background:#0b0d10; color:#e6e6e6; }
  header { padding: 16px; font-weight: 600; border-bottom: 1px solid #222; }
  .card { margin: 12px; padding: 12px; border:1px solid #222; border-radius: 10px; background:#12151a; }
  .badge { font-size: 12px; padding: 2px 8px; border-radius: 999px; background:#1e293b; }
  .muted { color:#8a8f98; font-size: 13px; }
</style></head>
<body>
  <header>Asylum · companion</header>
  <div id="notes"></div>
  <div id="projects"></div>
  <script>
    async function load() {
      const n = await (await fetch('/api/notifications')).json();
      document.getElementById('notes').innerHTML =
        '<div class="card"><b>Inbox</b> <span class="badge">' + n.unread + ' unread</span>' +
        n.items.slice(0,5).map(i => '<div class="muted">• ' + i.title + '</div>').join('') + '</div>';
      const ps = await (await fetch('/api/projects')).json();
      document.getElementById('projects').innerHTML =
        ps.map(p => '<div class="card">' + p.name + '</div>').join('');
    }
    load(); setInterval(load, 5000);
  </script>
</body></html>"#;

#[cfg(test)]
#[path = "../tests/router.rs"]
mod tests;
