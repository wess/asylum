use super::*;
use crate::model::QueueStatus;

fn seeded() -> (Db, i64) {
    let db = Db::memory().unwrap();
    let p = db.create_project("R", "/tmp/followup", "main", 1).unwrap();
    let t = db.create_task(p.id, "T", "prompt", 1).unwrap();
    (db, t.id)
}

#[test]
fn queue_claim_and_complete() {
    let (db, tid) = seeded();

    let first = db.queue_followup(tid, "ship it", "companion", 10).unwrap();
    db.queue_followup(tid, "and tests", "companion", 11)
        .unwrap();
    assert_eq!(first.status, QueueStatus::Pending);
    assert_eq!(first.attempts, 0);

    // Pending lists both, oldest first.
    let pending = db.pending_followups().unwrap();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].message, "ship it");

    // Claiming marks them running and bumps attempts; a second claim finds none.
    let claimed = db.claim_followups(20).unwrap();
    assert_eq!(claimed.len(), 2);
    assert_eq!(claimed[0].message, "ship it");
    assert_eq!(claimed[0].attempts, 1);
    assert_eq!(claimed[0].status, QueueStatus::Running);
    assert!(
        db.claim_followups(20).unwrap().is_empty(),
        "no double-claim"
    );
    assert!(db.pending_followups().unwrap().is_empty());

    db.complete_followup(first.id, 21).unwrap();
    assert_eq!(
        db.followup(first.id).unwrap().status,
        QueueStatus::Succeeded
    );
}

#[test]
fn a_backed_off_retry_waits_for_the_delay() {
    let (db, tid) = seeded();
    let f = db.queue_followup(tid, "later", "companion", 100).unwrap();
    // Claim (attempt 1) then fail transiently: pending again, backed off 5s.
    assert_eq!(db.claim_followups(100).unwrap().len(), 1);
    assert!(db.fail_followup(f.id, 100, "busy").unwrap(), "should retry");
    let row = db.followup(f.id).unwrap();
    assert_eq!(row.status, QueueStatus::Pending);
    assert_eq!(row.attempts, 1);
    assert_eq!(row.last_error.as_deref(), Some("busy"));
    // Not claimable until the backoff (base 5s) elapses.
    assert!(
        db.claim_followups(104).unwrap().is_empty(),
        "still backed off"
    );
    assert_eq!(db.claim_followups(105).unwrap().len(), 1, "delay elapsed");
}

#[test]
fn transient_failures_give_up_after_max_attempts() {
    let (db, tid) = seeded();
    let f = db
        .queue_followup(tid, "deliver me", "companion", 0)
        .unwrap();

    // Each round: claim (marks running, +1 attempt) then fail transiently.
    // Advance time past the max backoff so the next claim always succeeds.
    let mut t = 0i64;
    let mut attempts = 0;
    loop {
        let claimed = db.claim_followups(t).unwrap();
        assert_eq!(claimed.len(), 1, "claimable at t={t}");
        attempts += 1;
        let will_retry = db.fail_followup(f.id, t, "no live run").unwrap();
        let row = db.followup(f.id).unwrap();
        assert_eq!(row.attempts, attempts);
        if !will_retry {
            assert_eq!(row.status, QueueStatus::Failed, "terminal");
            break;
        }
        assert_eq!(row.status, QueueStatus::Pending);
        t += 1000; // beyond any backoff window
    }
    assert_eq!(attempts, 5, "gives up after max attempts");
    assert_eq!(
        db.followup(f.id).unwrap().last_error.as_deref(),
        Some("no live run")
    );
    // A failed row is never claimable again and is not "pending".
    assert!(db.pending_followups().unwrap().is_empty());
    assert!(db.claim_followups(t + 1_000_000).unwrap().is_empty());
}

#[test]
fn permanent_failure_is_terminal_immediately() {
    let (db, tid) = seeded();
    let f = db.queue_followup(tid, "bad", "companion", 0).unwrap();
    db.claim_followups(0).unwrap();
    db.fail_followup_permanent(f.id, 0, "task deleted").unwrap();
    let row = db.followup(f.id).unwrap();
    assert_eq!(row.status, QueueStatus::Failed);
    assert_eq!(row.attempts, 1, "no extra attempts consumed");
    assert_eq!(row.last_error.as_deref(), Some("task deleted"));
}

#[test]
fn crash_recovery_returns_stranded_running_rows() {
    let (db, tid) = seeded();
    let f = db
        .queue_followup(tid, "mid-flight", "companion", 0)
        .unwrap();
    // Claim it, then "crash" before completing: it stays running.
    db.claim_followups(0).unwrap();
    assert_eq!(db.followup(f.id).unwrap().status, QueueStatus::Running);

    // Too soon: within the stale window it is left alone.
    assert_eq!(db.recover_stale_followups(1).unwrap(), 0);
    assert_eq!(db.followup(f.id).unwrap().status, QueueStatus::Running);

    // After the stale window it is returned to pending and can be retried.
    let recovered = db.recover_stale_followups(10_000).unwrap();
    assert_eq!(recovered, 1);
    assert_eq!(db.followup(f.id).unwrap().status, QueueStatus::Pending);
    assert_eq!(db.claim_followups(10_000).unwrap().len(), 1);
}

#[test]
fn succeeded_rows_are_never_recovered_or_reclaimed() {
    let (db, tid) = seeded();
    let f = db.queue_followup(tid, "done", "companion", 0).unwrap();
    db.claim_followups(0).unwrap();
    db.complete_followup(f.id, 1).unwrap();
    // Recovery and re-claim leave a completed row untouched (no duplicate work).
    assert_eq!(db.recover_stale_followups(1_000_000).unwrap(), 0);
    assert!(db.claim_followups(1_000_000).unwrap().is_empty());
    assert_eq!(db.followup(f.id).unwrap().status, QueueStatus::Succeeded);
}
