use super::*;

fn db() -> Db {
    Db::memory().unwrap()
}

fn project(db: &Db) -> i64 {
    db.create_project("Repo", "/tmp/repo", "main", 100)
        .unwrap()
        .id
}

fn steps(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn a_routine_keeps_its_steps_in_order() {
    // Order is the whole content of a routine — a release run backwards is not
    // a release.
    let db = db();
    let p = project(&db);
    let saved = db
        .save_routine(
            p,
            "release",
            "cut a release",
            &steps(&["cargo test", "cargo build --release", "gh release create"]),
            0,
        )
        .unwrap();
    assert_eq!(
        saved.step_list(),
        vec!["cargo test", "cargo build --release", "gh release create"]
    );
}

#[test]
fn recording_again_replaces_rather_than_duplicates() {
    // Re-recording is how you fix a routine that was wrong; making somebody
    // delete the old one first turns "show it again" into two steps.
    let db = db();
    let p = project(&db);
    db.save_routine(p, "release", "", &steps(&["old"]), 0)
        .unwrap();
    db.save_routine(p, "release", "now correct", &steps(&["new", "newer"]), 1)
        .unwrap();

    let all = db.routines(p).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].step_list(), vec!["new", "newer"]);
    assert_eq!(all[0].description, "now correct");
}

#[test]
fn the_same_name_means_different_things_in_different_projects() {
    let db = db();
    let a = project(&db);
    let b = db
        .create_project("Other", "/tmp/other", "main", 100)
        .unwrap()
        .id;
    db.save_routine(a, "release", "", &steps(&["a"]), 0)
        .unwrap();
    db.save_routine(b, "release", "", &steps(&["b"]), 0)
        .unwrap();
    assert_eq!(db.routine(a, "release").unwrap().step_list(), vec!["a"]);
    assert_eq!(db.routine(b, "release").unwrap().step_list(), vec!["b"]);
}

#[test]
fn a_routine_with_unreadable_steps_has_none_rather_than_half() {
    // Replaying a partially-parsed sequence would run a prefix of somebody's
    // workflow and stop, which is worse than refusing to run it at all.
    let db = db();
    let p = project(&db);
    db.save_routine(p, "broken", "", &steps(&["ok"]), 0)
        .unwrap();
    db.conn()
        .execute(
            "UPDATE routines SET steps = 'not json' WHERE project_id = ?1",
            [p],
        )
        .unwrap();
    assert!(db.routine(p, "broken").unwrap().step_list().is_empty());
}

#[test]
fn an_empty_routine_is_allowed_and_simply_does_nothing() {
    // Recording a session in which you ran nothing is not an error worth
    // raising; it is an empty routine.
    let db = db();
    let p = project(&db);
    let r = db.save_routine(p, "empty", "", &[], 0).unwrap();
    assert!(r.step_list().is_empty());
}

#[test]
fn deleting_a_project_takes_its_routines() {
    let db = db();
    let p = project(&db);
    db.save_routine(p, "release", "", &steps(&["go"]), 0)
        .unwrap();
    db.delete_project(p).unwrap();
    assert!(db.routines(p).unwrap().is_empty());
}

#[test]
fn running_records_when() {
    let db = db();
    let p = project(&db);
    let r = db
        .save_routine(p, "release", "", &steps(&["go"]), 0)
        .unwrap();
    assert!(r.last_run_at.is_none());
    db.mark_routine_run(r.id, 500).unwrap();
    assert_eq!(db.routine(p, "release").unwrap().last_run_at, Some(500));
}
