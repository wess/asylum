use super::*;
use store::Db;

fn seeded() -> (Db, i64, i64) {
    let db = Db::memory().unwrap();
    let p = db.create_project("acme", "/tmp/acme", "main", 1).unwrap();
    let t = db.create_task(p.id, "Add login", "do it", 1).unwrap();
    db.create_run(t.id, "codex", "/wt/1", "b1").unwrap();
    db.notify("run_finished", "Codex done", "", None, 1)
        .unwrap();
    (db, p.id, t.id)
}

#[test]
fn health_ok() {
    let db = Db::memory().unwrap();
    let r = route("GET", "/api/health", "", &db);
    assert_eq!(r.status, 200);
    assert!(r.body.contains("\"ok\":true"));
}

#[test]
fn lists_projects_tasks_runs() {
    let (db, pid, tid) = seeded();
    let projects = route("GET", "/api/projects", "", &db);
    assert!(projects.body.contains("acme"));

    let tasks = route("GET", &format!("/api/projects/{pid}/tasks"), "", &db);
    assert!(tasks.body.contains("Add login"));

    let runs = route("GET", &format!("/api/tasks/{tid}/runs"), "", &db);
    assert!(runs.body.contains("codex"));
}

#[test]
fn notifications_include_unread_count() {
    let (db, _, _) = seeded();
    let r = route("GET", "/api/notifications", "", &db);
    assert!(r.body.contains("\"unread\":1"));
    assert!(r.body.contains("Codex done"));
}

#[test]
fn followup_records_a_notification() {
    let (db, _, tid) = seeded();
    let before = db.unread_count().unwrap();
    let r = route(
        "POST",
        &format!("/api/tasks/{tid}/followup"),
        r#"{"message":"also handle logout"}"#,
        &db,
    );
    assert_eq!(r.status, 200);
    assert_eq!(db.unread_count().unwrap(), before + 1);
    let latest = &db.notifications(false).unwrap()[0];
    assert_eq!(latest.kind, "followup");
    assert_eq!(latest.body, "also handle logout");

    // It is also queued for the desktop app to deliver to a run.
    let pending = db.pending_followups().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].task_id, tid);
    assert_eq!(pending[0].message, "also handle logout");
}

#[test]
fn root_serves_mobile_page() {
    let db = Db::memory().unwrap();
    let r = route("GET", "/", "", &db);
    assert_eq!(r.content_type, "text/html; charset=utf-8");
    assert!(r.body.contains("Asylum"));
}

#[test]
fn unknown_route_is_404() {
    let db = Db::memory().unwrap();
    assert_eq!(route("GET", "/nope", "", &db).status, 404);
    assert_eq!(
        route("POST", "/api/tasks/x/followup", "{}", &db).status,
        404
    );
}

#[test]
fn mutations_require_the_csrf_header() {
    // Reads never need it.
    assert!(csrf_ok("GET", None));
    assert!(csrf_ok("GET", Some("whatever")));
    // Mutations need the exact custom-header value.
    assert!(!csrf_ok("POST", None));
    assert!(!csrf_ok("POST", Some("0")));
    assert!(!csrf_ok("POST", Some("")));
    assert!(csrf_ok("POST", Some("1")));
    assert!(csrf_ok("POST", Some("  1  ")));
    assert!(!csrf_ok("PUT", None));
    assert!(!csrf_ok("DELETE", None));
    assert!(!csrf_ok("PATCH", None));
}

#[test]
fn options_is_405_and_never_cors() {
    let db = Db::memory().unwrap();
    assert_eq!(route("OPTIONS", "/api/projects", "", &db).status, 405);
    assert_eq!(route("OPTIONS", "/", "", &db).status, 405);
}

#[test]
fn page_script_builds_dom_without_html_interpolation() {
    let db = Db::memory().unwrap();
    let js = route("GET", "/app.js", "", &db);
    assert_eq!(js.status, 200);
    assert!(js.content_type.starts_with("application/javascript"));
    // Stored values reach the DOM only through textContent.
    assert!(js.body.contains("textContent"));
    assert!(!js.body.contains("innerHTML"));
    // The page carries no data-bearing markup and loads its script externally,
    // so a strict `script-src 'self'` CSP can apply.
    assert!(!MOBILE_HTML.contains("innerHTML"));
    assert!(MOBILE_HTML.contains("src=\"/app.js\""));
    assert!(MOBILE_HTML.contains("Content-Security-Policy"));
}

#[test]
fn stored_xss_payloads_survive_as_json_text_only() {
    // Project names and notification titles carrying markup, event handlers,
    // script terminators, SVG, and encoded markup round-trip as JSON string
    // values - never as document structure. Rendered via textContent on the
    // page, they can only ever display as literal characters.
    let db = Db::memory().unwrap();
    let payloads = [
        "<img src=x onerror=alert(1)>",
        "</script><script>alert(1)</script>",
        "<svg/onload=alert(1)>",
        "\"><b onmouseover=alert(1)>x",
        "&lt;script&gt;alert(1)&lt;/script&gt;",
    ];
    for (i, p) in payloads.iter().enumerate() {
        db.create_project(p, &format!("/tmp/p{i}"), "main", 1)
            .unwrap();
        db.notify("run_finished", p, "", None, 1).unwrap();
    }

    let projects = route("GET", "/api/projects", "", &db);
    let parsed: serde_json::Value = serde_json::from_str(&projects.body).unwrap();
    let names: Vec<&str> = parsed
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p["name"].as_str())
        .collect();
    for p in payloads {
        assert!(names.contains(&p), "missing payload {p:?} in {names:?}");
    }

    let notes = route("GET", "/api/notifications", "", &db);
    // The body must be valid JSON (payloads are escaped, not embedded as markup).
    let parsed: serde_json::Value = serde_json::from_str(&notes.body).unwrap();
    assert!(parsed["items"].is_array());
}

#[test]
fn strips_query_string() {
    let db = Db::memory().unwrap();
    assert_eq!(route("GET", "/api/health?ts=123", "", &db).status, 200);
}

#[test]
fn runs_carry_live_activity() {
    let (db, _pid, tid) = seeded();
    let run = &db.runs(tid).unwrap()[0];
    db.set_run_activity(run.id, Some("blocked")).unwrap();
    let r = route("GET", &format!("/api/tasks/{tid}/runs"), "", &db);
    assert!(r.body.contains("\"activity\":\"blocked\""));
}

#[test]
fn events_replay_from_a_cursor() {
    let db = Db::memory().unwrap();
    db.record_event("run_started", Some(1), Some(1), "", 1)
        .unwrap();
    let e2 = db
        .record_event("run_finished", Some(1), Some(1), "", 2)
        .unwrap();

    let all = route("GET", "/api/events?since=0", "", &db);
    assert_eq!(all.status, 200);
    assert!(all.body.contains("run_started"));
    assert!(all.body.contains(&format!("\"cursor\":{}", e2.id)));

    let tail = route("GET", &format!("/api/events?since={}", e2.id - 1), "", &db);
    assert!(tail.body.contains("run_finished"));
    assert!(!tail.body.contains("run_started"));
}
