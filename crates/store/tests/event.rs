use super::*;

#[test]
fn events_append_and_replay_from_cursor() {
    let db = Db::memory().unwrap();
    assert_eq!(db.latest_event_id().unwrap(), 0);

    let e1 = db
        .record_event("run_started", Some(1), Some(1), "{}", 10)
        .unwrap();
    let e2 = db
        .record_event(
            "run_activity",
            Some(1),
            Some(1),
            r#"{"activity":"working"}"#,
            11,
        )
        .unwrap();
    assert!(e2.id > e1.id);
    assert_eq!(db.latest_event_id().unwrap(), e2.id);

    // From the beginning.
    let all = db.events_since(0, 100).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].kind, "run_started");

    // From a cursor - only newer events.
    let tail = db.events_since(e1.id, 100).unwrap();
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].id, e2.id);
    assert_eq!(tail[0].kind, "run_activity");

    // Limit is honoured.
    let one = db.events_since(0, 1).unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].id, e1.id);
}

#[test]
fn pruning_keeps_only_the_most_recent() {
    let db = Db::memory().unwrap();
    for i in 0..10 {
        db.record_event("tick", None, None, "", 100 + i).unwrap();
    }
    let removed = db.prune_events(3).unwrap();
    assert_eq!(removed, 7);
    let remaining = db.events_since(0, 100).unwrap();
    assert_eq!(remaining.len(), 3);
}

#[test]
fn set_run_activity_round_trips() {
    let db = Db::memory().unwrap();
    let p = db.create_project("R", "/tmp/activity", "main", 1).unwrap();
    let t = db.create_task(p.id, "T", "prompt", 1).unwrap();
    let run = db
        .create_run(t.id, "claude-code", "/tmp/wt", "asylum/x")
        .unwrap();
    assert_eq!(run.activity, None);

    db.set_run_activity(run.id, Some("blocked")).unwrap();
    assert_eq!(db.run(run.id).unwrap().activity.as_deref(), Some("blocked"));

    db.set_run_activity(run.id, None).unwrap();
    assert_eq!(db.run(run.id).unwrap().activity, None);
}
