use super::*;
use crate::model::QueueStatus;

fn seeded() -> (Db, i64, i64) {
    let db = Db::memory().unwrap();
    let p = db.create_project("R", "/tmp/control", "main", 1).unwrap();
    let t = db.create_task(p.id, "T", "prompt", 1).unwrap();
    let run = db
        .create_run(t.id, "claude-code", "/tmp/wt", "asylum/x")
        .unwrap();
    (db, t.id, run.id)
}

#[test]
fn queue_claim_and_complete() {
    let (db, tid, run_id) = seeded();

    let first = db
        .queue_control_request(
            tid,
            Some(run_id),
            "spawn",
            r#"{"agent":"codex"}"#,
            "agent",
            10,
        )
        .unwrap();
    db.queue_control_request(tid, None, "check", "", "cli", 11)
        .unwrap();
    assert_eq!(first.status, QueueStatus::Pending);
    assert_eq!(first.attempts, 0);
    assert_eq!(first.run_id, Some(run_id));
    assert_eq!(first.kind, "spawn");

    let pending = db.pending_control_requests().unwrap();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].kind, "spawn");
    assert_eq!(pending[1].run_id, None);

    // Claim both atomically; a second claim yields nothing (no double-run).
    let claimed = db.claim_control_requests(20).unwrap();
    assert_eq!(claimed.len(), 2);
    assert_eq!(claimed[0].kind, "spawn");
    assert_eq!(claimed[0].attempts, 1);
    assert!(db.claim_control_requests(20).unwrap().is_empty());

    db.complete_control_request(first.id, 21).unwrap();
    assert_eq!(
        db.control_request(first.id).unwrap().status,
        QueueStatus::Succeeded
    );
}

#[test]
fn transient_failure_retries_then_terminal() {
    let (db, tid, _run) = seeded();
    let r = db
        .queue_control_request(tid, None, "spawn", r#"{"agent":"codex"}"#, "agent", 0)
        .unwrap();

    db.claim_control_requests(0).unwrap();
    assert!(db.fail_control_request(r.id, 0, "worktree busy").unwrap());
    let row = db.control_request(r.id).unwrap();
    assert_eq!(row.status, QueueStatus::Pending);
    assert_eq!(row.last_error.as_deref(), Some("worktree busy"));
    // Backed off: not claimable immediately.
    assert!(db.claim_control_requests(1).unwrap().is_empty());
    assert_eq!(db.claim_control_requests(10).unwrap().len(), 1);
}

#[test]
fn permanent_failure_does_not_retry() {
    let (db, tid, _run) = seeded();
    let r = db
        .queue_control_request(tid, None, "spawn", "{}", "agent", 0)
        .unwrap();
    db.claim_control_requests(0).unwrap();
    db.fail_control_request_permanent(r.id, 0, "unknown agent")
        .unwrap();
    let row = db.control_request(r.id).unwrap();
    assert_eq!(row.status, QueueStatus::Failed);
    assert_eq!(row.attempts, 1);
    assert!(db.claim_control_requests(1_000_000).unwrap().is_empty());
}

#[test]
fn crash_recovery_reclaims_stranded_work() {
    let (db, tid, run_id) = seeded();
    let r = db
        .queue_control_request(tid, Some(run_id), "check", "", "agent", 0)
        .unwrap();
    db.claim_control_requests(0).unwrap();
    assert_eq!(
        db.control_request(r.id).unwrap().status,
        QueueStatus::Running
    );
    // Recovered after the stale window and retried.
    assert_eq!(db.recover_stale_control_requests(10_000).unwrap(), 1);
    assert_eq!(
        db.control_request(r.id).unwrap().status,
        QueueStatus::Pending
    );
    assert_eq!(db.claim_control_requests(10_000).unwrap().len(), 1);
}

#[test]
fn missing_control_request_is_not_found() {
    let db = Db::memory().unwrap();
    assert!(matches!(db.control_request(999), Err(Error::NotFound)));
}
