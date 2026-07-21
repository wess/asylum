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
    pub fn js(body: &str) -> Self {
        Response {
            status: 200,
            content_type: "application/javascript; charset=utf-8".into(),
            body: body.to_string(),
        }
    }
}

/// Content-Security-Policy sent on every response. The page loads its script
/// from `/app.js` (`script-src 'self'`), so injected `<script>` tags and inline
/// event handlers are inert even before the DOM-building code renders stored
/// values as text. Everything else is same-origin only; framing is denied.
pub const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; object-src 'none'";

/// Whether `method` changes state (and so must clear the CSRF guard).
fn is_mutation(method: &str) -> bool {
    matches!(method, "POST" | "PUT" | "PATCH" | "DELETE")
}

/// Cross-site request-forgery guard. State-changing requests must carry the
/// custom header our own page sends; a browser cannot attach it to a cross-site
/// simple request without a CORS preflight this server never approves. Reads are
/// always allowed. Returns whether the request may proceed.
pub fn csrf_ok(method: &str, csrf_header: Option<&str>) -> bool {
    if !is_mutation(method) {
        return true;
    }
    csrf_header.map(str::trim) == Some("1")
}

/// Dispatch `(method, path, body)` against the store.
pub fn route(method: &str, path: &str, body: &str, db: &Db) -> Response {
    // Split off any query string; keep it for endpoints that read parameters.
    let (path, query) = match path.split_once('?') {
        Some((p, q)) => (p, q),
        None => (path, ""),
    };
    match (method, path) {
        ("GET", "/") => Response::html(MOBILE_HTML),
        ("GET", "/app.js") => Response::js(APP_JS),
        // We do not participate in CORS; answer preflights/other verbs with a
        // deliberate 405 and no `Access-Control-Allow-*` headers.
        ("OPTIONS", _) => Response::text(405, "method not allowed"),
        ("GET", "/api/health") => Response::json(200, json!({ "ok": true })),
        ("GET", "/api/projects") => projects(db),
        ("GET", "/api/notifications") => notifications(db),
        ("GET", "/api/events") => events(db, query),
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
    path.strip_prefix(prefix)?
        .strip_suffix(suffix)?
        .parse()
        .ok()
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
        .map(|r| {
            json!({
                "id": r.id,
                "agent": r.agent,
                "branch": r.branch,
                "status": r.status.as_str(),
                // Live semantic state, so the phone shows which agent is blocked.
                "activity": r.activity,
            })
        })
        .collect();
    Response::json(200, json!(list))
}

/// Replay the append-only event log from a `?since=<cursor>` position, so the
/// phone can follow the fleet without polling every table. `?limit=` caps the
/// page (default and max 200).
fn events(db: &Db, query: &str) -> Response {
    let since = query_param(query, "since")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let limit = query_param(query, "limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(200)
        .clamp(1, 200);
    let list = db.events_since(since, limit).unwrap_or_default();
    let cursor = list.last().map(|e| e.id).unwrap_or(since);
    let items: Vec<_> = list
        .into_iter()
        .map(|e| {
            json!({
                "id": e.id,
                "kind": e.kind,
                "task": e.task_id,
                "run": e.run_id,
                "at": e.created_at,
            })
        })
        .collect();
    Response::json(200, json!({ "cursor": cursor, "items": items }))
}

/// Extract a `key=value` from a `&`-joined query string.
fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then_some(v)
    })
}

fn notifications(db: &Db) -> Response {
    let list: Vec<_> = db
        .notifications(false)
        .unwrap_or_default()
        .into_iter()
        .map(|n| json!({ "id": n.id, "kind": n.kind, "title": n.title, "body": n.body, "read": n.read }))
        .collect();
    Response::json(
        200,
        json!({ "unread": db.unread_count().unwrap_or(0), "items": list }),
    )
}

/// Record a follow-up from the phone. The message is both queued for delivery
/// (the desktop app drains the queue and sends it to an active run) and posted
/// as a `followup` notification so it is visible in the inbox. As a mutation it
/// must clear the CSRF guard (the `X-Asylum-Companion` header) in
/// [`serve_on`](crate::serve_on).
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
    let now = now();
    if db
        .queue_followup(task_id, &message, "companion", now)
        .is_err()
    {
        return Response::text(500, "could not queue follow-up");
    }
    let _ = db.notify("followup", &task.title, &message, None, now);
    Response::json(200, json!({ "ok": true, "task": task_id }))
}

/// Whether a request carrying `auth` (`Authorization` header value) is allowed
/// when the server is configured with `token`. An empty configured token
/// denies every request - `config::bind::guard` should never let the server
/// start that way, but this check fails closed regardless rather than trust
/// that. Otherwise a matching `Bearer <token>` is required.
pub fn authorized(auth: Option<&str>, token: &str) -> bool {
    if token.trim().is_empty() {
        return false;
    }
    auth.and_then(|value| value.trim().strip_prefix("Bearer "))
        .is_some_and(|presented| presented.trim() == token)
}

/// Unix seconds. The companion runs on real wall-clock time.
fn now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The mobile status page. Self-contained; its script lives at `/app.js` so a
/// strict CSP can forbid inline script. Stored values (project names,
/// notification titles) are only ever written with `textContent`, never
/// interpolated into markup.
pub const MOBILE_HTML: &str = r#"<!doctype html>
<html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; object-src 'none'">
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
  <script src="/app.js"></script>
</body></html>"#;

/// The mobile page's script. Builds every node with `createElement` /
/// `textContent`, so a project name or notification title containing markup,
/// event handlers, or `</script>` renders as inert text - there is no HTML
/// interpolation path for stored data to reach the DOM as structure.
pub const APP_JS: &str = r#"'use strict';
async function j(u){ return (await fetch(u, {credentials:'same-origin'})).json(); }
function el(tag, cls, text){
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text != null) e.textContent = text;
  return e;
}
async function load(){
  const notes = document.getElementById('notes');
  const projects = document.getElementById('projects');
  try {
    const n = await j('/api/notifications');
    notes.replaceChildren();
    const card = el('div', 'card');
    card.append(el('b', null, 'Inbox'), document.createTextNode(' '),
                el('span', 'badge', (n.unread || 0) + ' unread'));
    (n.items || []).slice(0, 5).forEach(function(i){
      card.append(el('div', 'muted', '• ' + (i.title || '')));
    });
    notes.append(card);
    const ps = await j('/api/projects');
    projects.replaceChildren();
    (ps || []).forEach(function(p){ projects.append(el('div', 'card', p.name || '')); });
  } catch (e) { /* transient network/parse error; retry next tick */ }
}
load();
setInterval(load, 5000);
"#;

#[cfg(test)]
#[path = "../tests/router.rs"]
mod tests;
