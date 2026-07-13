use super::*;
use store::Db;

fn seeded() -> (Db, i64, i64) {
    let db = Db::memory().unwrap();
    let p = db.create_project("acme", "/tmp/acme", "main", 1).unwrap();
    let t = db.create_task(p.id, "Add login", "do it", 1).unwrap();
    db.create_run(t.id, "codex", "/wt/1", "b1").unwrap();
    db.notify("run_finished", "Codex done", "", None, 1).unwrap();
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
    assert_eq!(route("POST", "/api/tasks/x/followup", "{}", &db).status, 404);
}

#[test]
fn strips_query_string() {
    let db = Db::memory().unwrap();
    assert_eq!(route("GET", "/api/health?ts=123", "", &db).status, 200);
}
