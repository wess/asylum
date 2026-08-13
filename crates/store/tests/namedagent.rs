use super::*;

fn db() -> Db {
    Db::memory().unwrap()
}

fn project(db: &Db) -> i64 {
    db.create_project("Repo", "/tmp/repo", "main", 100)
        .unwrap()
        .id
}

#[test]
fn a_named_agent_survives_the_runs_it_does() {
    // The whole point: a run is a thing that happened, an agent is somebody who
    // keeps happening.
    let db = db();
    let p = project(&db);
    let a = db
        .save_named_agent(p, "Reviewer", "reviews diffs", "claude-code", 0)
        .unwrap();
    assert_eq!(a.name, "Reviewer");
    assert_eq!(db.named_agents(p).unwrap().len(), 1);
}

#[test]
fn changing_the_role_does_not_erase_what_it_learned() {
    // Re-running the command that created somebody is the most likely way to
    // lose their memory, so it must not.
    let db = db();
    let p = project(&db);
    let a = db
        .save_named_agent(p, "Reviewer", "old brief", "claude-code", 0)
        .unwrap();
    db.remember(a.id, "the flaky test is in probe.rs").unwrap();

    db.save_named_agent(p, "Reviewer", "a better brief", "codex", 1)
        .unwrap();
    let after = db.named_agent(p, "Reviewer").unwrap();
    assert_eq!(after.role, "a better brief");
    assert_eq!(after.agent_id, "codex");
    assert!(
        after.memory.contains("probe.rs"),
        "memory was erased: {:?}",
        after.memory
    );
}

#[test]
fn memory_accumulates_in_order() {
    let db = db();
    let p = project(&db);
    let a = db
        .save_named_agent(p, "Reviewer", "", "claude-code", 0)
        .unwrap();
    db.remember(a.id, "first").unwrap();
    db.remember(a.id, "second").unwrap();
    let memory = db.named_agent(p, "Reviewer").unwrap().memory;
    assert_eq!(memory, "- first\n- second");
}

#[test]
fn the_same_fact_is_not_learned_twice() {
    // An agent that writes the same line after every task would otherwise fill
    // its own memory with one fact until nothing else fits.
    let db = db();
    let p = project(&db);
    let a = db
        .save_named_agent(p, "Reviewer", "", "claude-code", 0)
        .unwrap();
    for _ in 0..5 {
        db.remember(a.id, "they only sign annual").unwrap();
    }
    assert_eq!(
        db.named_agent(p, "Reviewer").unwrap().memory,
        "- they only sign annual"
    );
}

#[test]
fn remembering_nothing_is_not_an_error() {
    let db = db();
    let p = project(&db);
    let a = db
        .save_named_agent(p, "Reviewer", "", "claude-code", 0)
        .unwrap();
    db.remember(a.id, "   ").unwrap();
    assert!(db.named_agent(p, "Reviewer").unwrap().memory.is_empty());
}

#[test]
fn forgetting_keeps_the_agent() {
    let db = db();
    let p = project(&db);
    let a = db
        .save_named_agent(p, "Reviewer", "reviews", "claude-code", 0)
        .unwrap();
    db.remember(a.id, "something wrong").unwrap();
    db.forget(a.id).unwrap();
    let after = db.named_agent(p, "Reviewer").unwrap();
    assert!(after.memory.is_empty());
    assert_eq!(after.role, "reviews", "forgetting is not firing");
}

#[test]
fn a_preamble_says_who_it_is_and_what_it_knows() {
    let db = db();
    let p = project(&db);
    let a = db
        .save_named_agent(
            p,
            "Reviewer",
            "You review diffs strictly.",
            "claude-code",
            0,
        )
        .unwrap();
    assert_eq!(
        a.preamble().unwrap(),
        "You are Reviewer. You review diffs strictly."
    );

    db.remember(a.id, "the flaky test is in probe.rs").unwrap();
    let with_memory = db.named_agent(p, "Reviewer").unwrap().preamble().unwrap();
    assert!(with_memory.contains("You are Reviewer."));
    assert!(with_memory.contains("probe.rs"));
}

#[test]
fn an_agent_with_nothing_to_say_contributes_no_preamble() {
    // Otherwise every prompt gains a line of boilerplate that teaches nothing.
    let db = db();
    let p = project(&db);
    let a = db
        .save_named_agent(p, "Blank", "", "claude-code", 0)
        .unwrap();
    assert!(a.preamble().is_none());
}

#[test]
fn a_name_means_something_different_in_each_project() {
    // "Reviewer" in a Rust workspace is not "Reviewer" on a marketing site.
    let db = db();
    let a = project(&db);
    let b = db
        .create_project("Site", "/tmp/site", "main", 100)
        .unwrap()
        .id;
    db.save_named_agent(a, "Reviewer", "rust", "claude-code", 0)
        .unwrap();
    db.save_named_agent(b, "Reviewer", "copy", "codex", 0)
        .unwrap();
    assert_eq!(db.named_agent(a, "Reviewer").unwrap().role, "rust");
    assert_eq!(db.named_agent(b, "Reviewer").unwrap().role, "copy");
}

#[test]
fn the_roster_is_ordered_by_who_you_used_last() {
    let db = db();
    let p = project(&db);
    db.save_named_agent(p, "Alpha", "", "claude-code", 0)
        .unwrap();
    let b = db
        .save_named_agent(p, "Bravo", "", "claude-code", 0)
        .unwrap();
    db.touch_named_agent(b.id, 500).unwrap();
    assert_eq!(db.named_agents(p).unwrap()[0].name, "Bravo");
}

#[test]
fn deleting_a_project_takes_its_roster() {
    let db = db();
    let p = project(&db);
    db.save_named_agent(p, "Reviewer", "", "claude-code", 0)
        .unwrap();
    db.delete_project(p).unwrap();
    assert!(db.named_agents(p).unwrap().is_empty());
}

#[test]
fn a_run_can_be_attributed_to_somebody() {
    let db = db();
    let p = project(&db);
    let t = db.create_task(p, "Task", "do it", 0).unwrap();
    let r = db.create_run(t.id, "claude-code", "/tmp/wt", "wt").unwrap();
    assert!(
        db.run_agent(r.id).unwrap().is_none(),
        "runs are anonymous by default"
    );

    let a = db
        .save_named_agent(p, "Reviewer", "", "claude-code", 0)
        .unwrap();
    db.assign_run_agent(r.id, a.id, 900).unwrap();
    assert_eq!(db.run_agent(r.id).unwrap().unwrap().name, "Reviewer");
    // Being used is what puts somebody at the top of the roster.
    assert_eq!(
        db.named_agent(p, "Reviewer").unwrap().last_used_at,
        Some(900)
    );
}

#[test]
fn firing_somebody_does_not_delete_their_work() {
    // ON DELETE SET NULL, not CASCADE: the runs are the record of what happened
    // and must outlive the roster entry.
    let db = db();
    let p = project(&db);
    let t = db.create_task(p, "Task", "do it", 0).unwrap();
    let r = db.create_run(t.id, "claude-code", "/tmp/wt", "wt").unwrap();
    let a = db
        .save_named_agent(p, "Reviewer", "", "claude-code", 0)
        .unwrap();
    db.assign_run_agent(r.id, a.id, 0).unwrap();

    db.delete_named_agent(p, "Reviewer").unwrap();
    assert!(db.run(r.id).is_ok(), "the run went with them");
    assert!(db.run_agent(r.id).unwrap().is_none());
}
