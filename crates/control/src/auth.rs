//! Request authorization for the control surface: verify the scoped token and
//! confine the caller to its own task.
//!
//! A valid credential names a task (see [`crate::token`]); this module maps each
//! request to the task it targets and refuses one that targets a different task,
//! so a run-scoped credential cannot operate across the fleet. Endpoints not tied
//! to a single task (health, the event stream) are allowed for any valid token.

use store::Db;

use crate::token;

/// Authorize a request. An empty `key` disables authentication (used only when
/// the control server is intentionally unauthenticated). Otherwise a valid
/// scoped bearer token is required (`401` if missing/invalid/expired) whose task
/// matches the request's target task (`403` otherwise). `Ok` means proceed.
pub fn authorize(auth: Option<&str>, key: &str, path: &str, now: i64, db: &Db) -> Result<(), u16> {
    if key.is_empty() {
        return Ok(());
    }
    let Some(bearer) = bearer(auth) else {
        return Err(401);
    };
    let Some(scope) = token::verify(bearer, key, now) else {
        return Err(401);
    };
    match target_task(path, db) {
        Some(task) if task != scope.task_id => Err(403),
        _ => Ok(()),
    }
}

/// Extract the `Bearer <token>` value from an `Authorization` header.
fn bearer(auth: Option<&str>) -> Option<&str> {
    auth.and_then(|v| v.trim().strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// The specific task a request targets, if any. A run path resolves through the
/// store to its run's task. Endpoints not bound to one task (health, events)
/// return `None` and are allowed for any valid credential.
fn target_task(full_path: &str, db: &Db) -> Option<i64> {
    let (path, query) = match full_path.split_once('?') {
        Some((p, q)) => (p, q),
        None => (full_path, ""),
    };
    // `GET /control/runs?task=<id>`
    if path == "/control/runs" {
        return query_param(query, "task").and_then(|v| v.parse().ok());
    }
    // `POST /control/tasks/<id>/spawn`
    if let Some(id) = path
        .strip_prefix("/control/tasks/")
        .and_then(|p| p.strip_suffix("/spawn"))
        .and_then(|s| s.parse().ok())
    {
        return Some(id);
    }
    // Any `/control/runs/<id>...` resolves to that run's task.
    if let Some(rest) = path.strip_prefix("/control/runs/") {
        let id_str = rest.split('/').next().unwrap_or(rest);
        if let Ok(run_id) = id_str.parse::<i64>() {
            // A missing run has no known task; let routing return 404 rather
            // than reveal cross-task existence via a 403.
            return db.run(run_id).ok().map(|r| r.task_id);
        }
    }
    None
}

fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then_some(v)
    })
}

#[cfg(test)]
#[path = "../tests/auth.rs"]
mod tests;
