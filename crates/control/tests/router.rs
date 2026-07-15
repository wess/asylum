use super::*;
use serde_json::Value;
use store::Db;

fn seed() -> (Db, i64, i64) {
    let db = Db::memory().unwrap();
    let p = db
        .create_project("R", "/tmp/control-router", "main", 1)
        .unwrap();
    let t = db.create_task(p.id, "T", "do the thing", 1).unwrap();
    let run = db
        .create_run(t.id, "claude-code", "/tmp/wt", "asylum/x")
        .unwrap();
    db.start_run(run.id, 5).unwrap();
    db.save_run_output(run.id, "line one\nline two\nProceed? (y/n)")
        .unwrap();
    (db, t.id, run.id)
}

fn body(r: &Response) -> Value {
    serde_json::from_str(&r.body).unwrap()
}

#[test]
fn health_is_ok() {
    let db = Db::memory().unwrap();
    let r = route("GET", "/control/health", "", 1, &db);
    assert_eq!(r.status, 200);
    assert_eq!(body(&r)["ok"], true);
}

#[test]
fn siblings_list_carries_activity() {
    let (db, task, run) = seed();
    db.set_run_activity(run, Some("blocked")).unwrap();
    let r = route("GET", &format!("/control/runs?task={task}"), "", 1, &db);
    assert_eq!(r.status, 200);
    let v = body(&r);
    assert_eq!(v["runs"].as_array().unwrap().len(), 1);
    assert_eq!(v["runs"][0]["agent"], "claude-code");
    assert_eq!(v["runs"][0]["activity"], "blocked");
}

#[test]
fn runs_without_task_param_is_a_400() {
    let db = Db::memory().unwrap();
    assert_eq!(route("GET", "/control/runs", "", 1, &db).status, 400);
}

#[test]
fn one_run_returns_a_transcript_tail() {
    let (db, _task, run) = seed();
    let r = route("GET", &format!("/control/runs/{run}"), "", 1, &db);
    assert_eq!(r.status, 200);
    let v = body(&r);
    assert_eq!(v["id"], run);
    assert!(v["output_tail"].as_str().unwrap().contains("Proceed?"));
}

#[test]
fn a_missing_run_is_404() {
    let db = Db::memory().unwrap();
    assert_eq!(route("GET", "/control/runs/999", "", 1, &db).status, 404);
}

#[test]
fn reporting_activity_persists_and_logs_an_event() {
    let (db, _task, run) = seed();
    let r = route(
        "POST",
        &format!("/control/runs/{run}/activity"),
        r#"{"activity":"done"}"#,
        42,
        &db,
    );
    assert_eq!(r.status, 200);
    assert_eq!(db.run(run).unwrap().activity.as_deref(), Some("done"));
    let events = db.events_since(0, 10).unwrap();
    assert!(events.iter().any(|e| e.kind == "run_activity"));
}

#[test]
fn reporting_activity_requires_the_field() {
    let (db, _task, run) = seed();
    let r = route(
        "POST",
        &format!("/control/runs/{run}/activity"),
        "{}",
        1,
        &db,
    );
    assert_eq!(r.status, 400);
}

#[test]
fn spawn_queues_a_control_request() {
    let (db, task, _run) = seed();
    let r = route(
        "POST",
        &format!("/control/tasks/{task}/spawn"),
        r#"{"agent":"codex","prompt":"write tests"}"#,
        7,
        &db,
    );
    assert_eq!(r.status, 200);
    let pending = db.pending_control_requests().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].kind, "spawn");
    assert!(pending[0].payload.contains("codex"));
}

#[test]
fn spawn_requires_an_agent() {
    let (db, task, _run) = seed();
    let r = route(
        "POST",
        &format!("/control/tasks/{task}/spawn"),
        "{}",
        1,
        &db,
    );
    assert_eq!(r.status, 400);
}

#[test]
fn queueing_a_check_records_the_run() {
    let (db, _task, run) = seed();
    let r = route("POST", &format!("/control/runs/{run}/check"), "", 1, &db);
    assert_eq!(r.status, 200);
    let pending = db.pending_control_requests().unwrap();
    assert_eq!(pending[0].kind, "check");
    assert_eq!(pending[0].run_id, Some(run));
}

#[test]
fn events_replay_from_a_cursor() {
    let (db, task, run) = seed();
    db.record_event("run_started", Some(task), Some(run), "", 1)
        .unwrap();
    let e2 = db
        .record_event("run_finished", Some(task), Some(run), "", 2)
        .unwrap();
    let r = route("GET", "/control/events?since=0&limit=100", "", 1, &db);
    let v = body(&r);
    assert_eq!(v["cursor"], e2.id);
    assert!(v["events"].as_array().unwrap().len() >= 2);
}

#[test]
fn unknown_route_is_404() {
    let db = Db::memory().unwrap();
    assert_eq!(route("GET", "/control/nope", "", 1, &db).status, 404);
}
