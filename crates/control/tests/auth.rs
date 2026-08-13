use super::*;
use crate::token;
use store::Db;

/// A store with two tasks, each with a run; returns their ids.
fn seeded() -> (Db, i64, i64, i64, i64) {
    let db = Db::memory().unwrap();
    let p = db.create_project("R", "/tmp/auth", "main", 1).unwrap();
    let t1 = db.create_task(p.id, "T1", "p", 1).unwrap();
    let t2 = db.create_task(p.id, "T2", "p", 1).unwrap();
    let r1 = db.create_run(t1.id, "claude-code", "/wt/1", "b1").unwrap();
    let r2 = db.create_run(t2.id, "claude-code", "/wt/2", "b2").unwrap();
    (db, t1.id, r1.id, t2.id, r2.id)
}

fn bearer(tok: &str) -> String {
    format!("Bearer {tok}")
}

#[test]
fn empty_key_disables_auth() {
    let (db, t1, _r1, _t2, _r2) = seeded();
    assert_eq!(
        authorize(None, "", &format!("/control/runs?task={t1}"), 0, &db),
        Ok(())
    );
}

#[test]
fn missing_or_invalid_token_is_401() {
    let (db, t1, _r1, _t2, _r2) = seeded();
    let path = format!("/control/runs?task={t1}");
    // Missing.
    assert_eq!(authorize(None, "key", &path, 0, &db), Err(401));
    // Not a bearer / garbage.
    assert_eq!(authorize(Some("Basic x"), "key", &path, 0, &db), Err(401));
    assert_eq!(
        authorize(Some(&bearer("nonsense")), "key", &path, 0, &db),
        Err(401)
    );
}

#[test]
fn expired_token_is_401() {
    let (db, t1, r1, _t2, _r2) = seeded();
    let tok = token::mint("key", t1, r1, 500);
    let path = format!("/control/runs?task={t1}");
    assert_eq!(
        authorize(Some(&bearer(&tok)), "key", &path, 600, &db),
        Err(401)
    );
    // ...but valid before expiry.
    assert_eq!(
        authorize(Some(&bearer(&tok)), "key", &path, 100, &db),
        Ok(())
    );
}

#[test]
fn in_scope_requests_are_allowed() {
    let (db, t1, r1, _t2, _r2) = seeded();
    let tok = token::mint("key", t1, r1, 0);
    let auth = bearer(&tok);
    // Its own task list, its own run, spawn on its own task.
    assert_eq!(
        authorize(
            Some(&auth),
            "key",
            &format!("/control/runs?task={t1}"),
            0,
            &db
        ),
        Ok(())
    );
    assert_eq!(
        authorize(Some(&auth), "key", &format!("/control/runs/{r1}"), 0, &db),
        Ok(())
    );
    assert_eq!(
        authorize(
            Some(&auth),
            "key",
            &format!("/control/runs/{r1}/checks"),
            0,
            &db
        ),
        Ok(())
    );
    assert_eq!(
        authorize(
            Some(&auth),
            "key",
            &format!("/control/tasks/{t1}/spawn"),
            0,
            &db
        ),
        Ok(())
    );
    // Non-task-scoped endpoints are allowed for any valid credential.
    assert_eq!(
        authorize(Some(&auth), "key", "/control/events", 0, &db),
        Ok(())
    );
}

#[test]
fn cross_task_requests_are_403() {
    let (db, t1, r1, t2, r2) = seeded();
    let tok = token::mint("key", t1, r1, 0);
    let auth = bearer(&tok);
    // Another task's run, checks, list, and spawn are all refused.
    assert_eq!(
        authorize(Some(&auth), "key", &format!("/control/runs/{r2}"), 0, &db),
        Err(403)
    );
    assert_eq!(
        authorize(
            Some(&auth),
            "key",
            &format!("/control/runs/{r2}/activity"),
            0,
            &db
        ),
        Err(403)
    );
    assert_eq!(
        authorize(
            Some(&auth),
            "key",
            &format!("/control/runs?task={t2}"),
            0,
            &db
        ),
        Err(403)
    );
    assert_eq!(
        authorize(
            Some(&auth),
            "key",
            &format!("/control/tasks/{t2}/spawn"),
            0,
            &db
        ),
        Err(403)
    );
}

#[test]
fn a_sibling_may_not_write_to_another_agents_memory() {
    // Memory is the one thing on the surface that outlives the task: a line
    // written there is read at the start of every future run in the project,
    // so a sibling writing it could plant an instruction that survives long
    // after the run that planted it is gone.
    let db = Db::memory().unwrap();
    let p = db
        .create_project("R", "/tmp/auth-memory", "main", 1)
        .unwrap();
    let t = db.create_task(p.id, "T", "do it", 1).unwrap();
    let mine = db.create_run(t.id, "claude-code", "/tmp/a", "a").unwrap();
    let theirs = db.create_run(t.id, "codex", "/tmp/b", "b").unwrap();

    let key = "k";
    let token = token::mint(key, t.id, mine.id, 0);
    let auth = format!("Bearer {token}");

    // My own memory: allowed.
    assert!(authorize(
        Some(&auth),
        key,
        &format!("/control/runs/{}/remember", mine.id),
        1,
        &db
    )
    .is_ok());
    // A sibling's, on the same task: refused.
    assert_eq!(
        authorize(
            Some(&auth),
            key,
            &format!("/control/runs/{}/remember", theirs.id),
            1,
            &db
        ),
        Err(403)
    );
    // Reading that same sibling stays allowed — coordination is the point.
    assert!(authorize(
        Some(&auth),
        key,
        &format!("/control/runs/{}", theirs.id),
        1,
        &db
    )
    .is_ok());
}
