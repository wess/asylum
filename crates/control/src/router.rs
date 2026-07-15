//! Request routing for the agent control surface - pure over a [`store::Db`],
//! so the whole API is tested without sockets.
//!
//! Endpoints (all under `/control`):
//!
//! | Method + path                       | Effect                                   |
//! |-------------------------------------|------------------------------------------|
//! | `GET  /control/health`              | liveness                                 |
//! | `GET  /control/runs?task=<id>`      | sibling runs of a task, with activity    |
//! | `GET  /control/runs/<id>`           | one run + a tail of its transcript       |
//! | `GET  /control/runs/<id>/checks`    | that run's verification results          |
//! | `POST /control/runs/<id>/activity`  | self-report semantic state               |
//! | `POST /control/runs/<id>/check`     | queue a checks pass in the worktree      |
//! | `POST /control/tasks/<id>/spawn`    | queue a helper run (agent + prompt)      |
//! | `GET  /control/events?since=<id>`   | replay the event log from a cursor       |
//!
//! Reads answer directly from the store. Writes that need git/pty side-effects
//! (spawn a run, run checks) are *queued* as [`store::ControlRequest`]s and
//! drained by the desktop app, exactly like mobile follow-ups - which keeps this
//! router a pure function.

use serde_json::{json, Value};
use store::Db;

/// A ready-to-write HTTP response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub content_type: String,
    pub body: String,
}

impl Response {
    pub fn json(status: u16, value: Value) -> Self {
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
}

/// How many trailing lines of a transcript a run read returns.
const TAIL_LINES: usize = 40;
/// Default and ceiling for an events page.
const EVENTS_LIMIT: i64 = 200;

/// Dispatch `(method, path, body)` against the store. `now` is the wall-clock
/// stamp for any rows written (injected so the router stays pure).
pub fn route(method: &str, path: &str, body: &str, now: i64, db: &Db) -> Response {
    let (path, query) = split_query(path);
    match (method, path) {
        ("GET", "/control/health") => Response::json(200, json!({ "ok": true })),
        ("GET", "/control/runs") => match query_param(query, "task").and_then(|v| v.parse().ok()) {
            Some(task_id) => sibling_runs(db, task_id),
            None => Response::text(400, "runs requires ?task=<id>"),
        },
        ("GET", "/control/events") => events(db, query),
        ("GET", p) if p.starts_with("/control/runs/") && p.ends_with("/checks") => {
            with_id(p, "/control/runs/", "/checks", |id| run_checks(db, id))
        }
        ("GET", p) if p.starts_with("/control/runs/") => {
            with_id(p, "/control/runs/", "", |id| one_run(db, id))
        }
        ("POST", p) if p.starts_with("/control/runs/") && p.ends_with("/activity") => {
            with_id(p, "/control/runs/", "/activity", |id| {
                report_activity(db, id, body, now)
            })
        }
        ("POST", p) if p.starts_with("/control/runs/") && p.ends_with("/check") => {
            with_id(p, "/control/runs/", "/check", |id| queue_check(db, id, now))
        }
        ("POST", p) if p.starts_with("/control/tasks/") && p.ends_with("/spawn") => {
            with_id(p, "/control/tasks/", "/spawn", |id| {
                spawn(db, id, body, now)
            })
        }
        _ => Response::text(404, "not found"),
    }
}

fn sibling_runs(db: &Db, task_id: i64) -> Response {
    let list: Vec<Value> = db
        .runs(task_id)
        .unwrap_or_default()
        .into_iter()
        .map(|r| run_summary(&r))
        .collect();
    Response::json(200, json!({ "task": task_id, "runs": list }))
}

fn one_run(db: &Db, id: i64) -> Response {
    match db.run(id) {
        Ok(r) => {
            let mut v = run_summary(&r);
            v["output_tail"] = json!(tail(&r.output, TAIL_LINES));
            v["exit_code"] = json!(r.exit_code);
            v["error"] = json!(r.error);
            Response::json(200, v)
        }
        Err(_) => Response::text(404, "no such run"),
    }
}

fn run_checks(db: &Db, id: i64) -> Response {
    if db.run(id).is_err() {
        return Response::text(404, "no such run");
    }
    let list: Vec<Value> = db
        .run_checks(id)
        .unwrap_or_default()
        .into_iter()
        .map(|c| {
            json!({
                "id": c.id,
                "status": c.status,
                "summary": c.summary,
                "duration_ms": c.duration_ms,
            })
        })
        .collect();
    Response::json(200, json!({ "run": id, "checks": list }))
}

fn report_activity(db: &Db, id: i64, body: &str, now: i64) -> Response {
    let Ok(run) = db.run(id) else {
        return Response::text(404, "no such run");
    };
    let Some(activity) = field(body, "activity") else {
        return Response::text(400, "activity is required");
    };
    if db.set_run_activity(id, Some(&activity)).is_err() {
        return Response::text(500, "could not set activity");
    }
    let _ = db.record_event(
        "run_activity",
        Some(run.task_id),
        Some(id),
        &json!({ "activity": activity }).to_string(),
        now,
    );
    Response::json(200, json!({ "ok": true, "run": id, "activity": activity }))
}

fn queue_check(db: &Db, id: i64, now: i64) -> Response {
    let Ok(run) = db.run(id) else {
        return Response::text(404, "no such run");
    };
    match db.queue_control_request(run.task_id, Some(id), "check", "", "agent", now) {
        Ok(req) => {
            let _ = db.record_event("control_check", Some(run.task_id), Some(id), "", now);
            Response::json(200, json!({ "queued": req.id, "kind": "check" }))
        }
        Err(_) => Response::text(500, "could not queue check"),
    }
}

fn spawn(db: &Db, task_id: i64, body: &str, now: i64) -> Response {
    if db.task(task_id).is_err() {
        return Response::text(404, "no such task");
    }
    let Some(agent) = field(body, "agent").filter(|a| !a.is_empty()) else {
        return Response::text(400, "agent is required");
    };
    let prompt = field(body, "prompt");
    let from_run = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("from_run").and_then(Value::as_i64));
    let payload = json!({ "agent": agent, "prompt": prompt }).to_string();
    match db.queue_control_request(task_id, from_run, "spawn", &payload, "agent", now) {
        Ok(req) => {
            let _ = db.record_event("control_spawn", Some(task_id), from_run, &payload, now);
            Response::json(
                200,
                json!({ "queued": req.id, "kind": "spawn", "agent": agent }),
            )
        }
        Err(_) => Response::text(500, "could not queue spawn"),
    }
}

fn events(db: &Db, query: &str) -> Response {
    let since = query_param(query, "since")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let limit = query_param(query, "limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(EVENTS_LIMIT)
        .clamp(1, EVENTS_LIMIT);
    let list = db.events_since(since, limit).unwrap_or_default();
    let cursor = list.last().map(|e| e.id).unwrap_or(since);
    let items: Vec<Value> = list
        .into_iter()
        .map(|e| {
            json!({
                "id": e.id,
                "kind": e.kind,
                "task": e.task_id,
                "run": e.run_id,
                "data": e.data,
                "at": e.created_at,
            })
        })
        .collect();
    Response::json(200, json!({ "cursor": cursor, "events": items }))
}

fn run_summary(r: &store::Run) -> Value {
    json!({
        "id": r.id,
        "task": r.task_id,
        "agent": r.agent,
        "branch": r.branch,
        "status": r.status.as_str(),
        "activity": r.activity,
        "attempt": r.attempt,
    })
}

/// Parse a JSON body and pull a top-level string field.
fn field(body: &str, key: &str) -> Option<String> {
    serde_json::from_str::<Value>(body)
        .ok()?
        .get(key)?
        .as_str()
        .map(str::to_string)
}

/// Split `path` into `(path, query)` on the first `?`.
fn split_query(full: &str) -> (&str, &str) {
    match full.split_once('?') {
        Some((p, q)) => (p, q),
        None => (full, ""),
    }
}

/// Extract a `key=value` from a `&`-joined query string.
fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then_some(v)
    })
}

/// Parse the id embedded between `prefix` and `suffix`, then run `f`.
fn with_id(path: &str, prefix: &str, suffix: &str, f: impl FnOnce(i64) -> Response) -> Response {
    match path
        .strip_prefix(prefix)
        .and_then(|p| p.strip_suffix(suffix))
        .and_then(|s| s.parse().ok())
    {
        Some(id) => f(id),
        None => Response::text(404, "not found"),
    }
}

/// The last `lines` lines of `output`.
fn tail(output: &str, lines: usize) -> String {
    let all: Vec<&str> = output.lines().collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

#[cfg(test)]
#[path = "../tests/router.rs"]
mod tests;
