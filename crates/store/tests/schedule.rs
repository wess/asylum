use super::*;
use crate::schedule::NewSchedule;

fn db() -> Db {
    Db::memory().unwrap()
}

fn project(db: &Db) -> i64 {
    db.create_project("Repo", "/tmp/repo", "main", 100)
        .unwrap()
        .id
}

fn add(db: &Db, project_id: i64, title: &str, every_minutes: i64, first_at: i64) -> Schedule {
    db.create_schedule(
        NewSchedule {
            project_id,
            title,
            prompt: "go",
            agents: "",
            every_minutes,
            first_at,
        },
        100,
    )
    .unwrap()
}

// `advance` is the whole of the timing behaviour, and the part that decides
// whether a laptop closed over a weekend comes back to one run or thirty.

#[test]
fn a_schedule_that_is_on_time_simply_steps_forward() {
    assert_eq!(advance(1_000, 10, 1_000), 1_000 + 600);
}

#[test]
fn missed_periods_are_skipped_not_replayed() {
    // Dormant for a day at an hourly cadence. The useful behaviour is one run
    // now and the cadence resumed — not 24 fan-outs at once for work that has
    // since been done twice over.
    let next = advance(0, 60, 86_400);
    assert!(next > 86_400, "next must be in the future: {next}");
    assert!(next <= 86_400 + 3_600, "and within one cadence: {next}");
}

#[test]
fn the_next_time_is_always_in_the_future() {
    // Otherwise a due schedule is picked up again on the very next tick, and
    // one broken run becomes a machine starting another every minute.
    for behind in [0, 1, 59, 60, 61, 100_000] {
        let next = advance(0, 1, behind);
        assert!(next > behind, "behind={behind} gave next={next}");
    }
}

#[test]
fn a_zero_cadence_cannot_produce_a_stuck_schedule() {
    assert_eq!(advance(0, 0, 0), 60);
}

#[test]
fn due_finds_only_enabled_schedules_that_have_come_round() {
    let db = db();
    let p = project(&db);
    let soon = add(&db, p, "nightly", 1440, 500);
    let later = add(&db, p, "weekly", 10080, 5_000);

    assert_eq!(db.due_schedules(400).unwrap().len(), 0, "nothing due yet");
    let due = db.due_schedules(600).unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].id, soon.id);

    // Disabled means never due, however overdue.
    db.set_schedule_enabled(soon.id, false).unwrap();
    let due = db.due_schedules(100_000).unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].id, later.id);
}

#[test]
fn firing_moves_it_on_and_records_when() {
    let db = db();
    let p = project(&db);
    let s = add(&db, p, "nightly", 60, 1_000);
    db.mark_schedule_fired(s.id, 1_000).unwrap();

    let after = db.schedule(s.id).unwrap();
    assert_eq!(after.last_run_at, Some(1_000));
    assert_eq!(after.next_at, 1_000 + 3_600);
    // And no longer due at the moment it fired, which is what stops the next
    // tick picking it straight back up.
    assert!(db.due_schedules(1_000).unwrap().is_empty());
}

#[test]
fn agents_are_a_list_and_empty_means_the_projects_defaults() {
    let db = db();
    let p = project(&db);
    let named = db
        .create_schedule(
            NewSchedule {
                project_id: p,
                title: "a",
                prompt: "go",
                agents: "claude-code, codex",
                every_minutes: 60,
                first_at: 0,
            },
            0,
        )
        .unwrap();
    assert_eq!(named.agent_ids(), vec!["claude-code", "codex"]);
    assert!(add(&db, p, "b", 60, 0).agent_ids().is_empty());
}

#[test]
fn deleting_a_project_takes_its_schedules() {
    let db = db();
    let p = project(&db);
    add(&db, p, "a", 60, 0);
    db.delete_project(p).unwrap();
    // A schedule pointing at a project that no longer exists would fire into
    // nothing, forever.
    assert!(db.due_schedules(100_000).unwrap().is_empty());
}
