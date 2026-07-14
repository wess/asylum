use super::*;

#[test]
fn checks_replace_as_one_run_snapshot() {
    let db = Db::memory().unwrap();
    let project = db.create_project("p", "/tmp/checks", "main", 1).unwrap();
    let task = db.create_task(project.id, "t", "p", 1).unwrap();
    let run = db.create_run(task.id, "a", "/tmp/w", "b").unwrap();
    db.replace_run_checks(
        run.id,
        &[RunCheck {
            run_id: run.id,
            id: "test".into(),
            status: "pass".into(),
            summary: "ok".into(),
            duration_ms: 12,
        }],
    )
    .unwrap();
    assert_eq!(db.run_checks(run.id).unwrap()[0].status, "pass");
    db.replace_run_checks(run.id, &[]).unwrap();
    assert!(db.run_checks(run.id).unwrap().is_empty());
}
