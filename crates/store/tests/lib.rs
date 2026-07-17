use super::*;
use crate::model::{RunStatus, TaskStatus};

fn db() -> Db {
    Db::memory().unwrap()
}

#[test]
fn project_create_is_idempotent_by_path() {
    let db = db();
    let a = db.create_project("Repo", "/tmp/repo", "main", 100).unwrap();
    let b = db
        .create_project("Repo again", "/tmp/repo", "main", 200)
        .unwrap();
    assert_eq!(a.id, b.id);
    assert_eq!(db.projects().unwrap().len(), 1);
}

#[test]
fn task_lifecycle() {
    let db = db();
    let p = db.create_project("R", "/tmp/r", "main", 1).unwrap();
    let t = db
        .create_task(p.id, "Add feature", "do the thing", 10)
        .unwrap();
    assert_eq!(t.status, TaskStatus::Draft);

    db.set_task_status(t.id, TaskStatus::Review, 20).unwrap();
    let t = db.task(t.id).unwrap();
    assert_eq!(t.status, TaskStatus::Review);
    assert_eq!(t.updated_at, 20);

    assert_eq!(db.tasks(p.id).unwrap().len(), 1);
}

#[test]
fn run_fanout_and_finish() {
    let db = db();
    let p = db.create_project("R", "/tmp/r2", "main", 1).unwrap();
    let t = db.create_task(p.id, "T", "prompt", 1).unwrap();

    let r1 = db
        .create_run(t.id, "claude-code", "/wt/1", "task-1a")
        .unwrap();
    let r2 = db.create_run(t.id, "codex", "/wt/2", "task-1b").unwrap();
    assert_eq!(db.runs(t.id).unwrap().len(), 2);
    assert_eq!(r1.status, RunStatus::Queued);

    db.start_run(r1.id, 5).unwrap();
    db.finish_run(r1.id, 0, 9).unwrap();
    assert_eq!(db.successful_agents().unwrap(), vec!["claude-code"]);
    let r1 = db.run(r1.id).unwrap();
    assert_eq!(r1.status, RunStatus::Succeeded);
    assert_eq!(r1.started_at, Some(5));
    assert_eq!(r1.ended_at, Some(9));
    assert_eq!(r1.exit_code, Some(0));
    assert!(r1.status.is_terminal());

    db.finish_run(r2.id, 1, 12).unwrap();
    assert_eq!(db.run(r2.id).unwrap().status, RunStatus::Failed);
}

#[test]
fn run_output_retry_queue_and_recovery() {
    let db = Db::memory().unwrap();
    let project = db.create_project("p", "/tmp/p", "main", 1).unwrap();
    let task = db.create_task(project.id, "t", "prompt", 2).unwrap();
    let run = db.create_run(task.id, "agent", "/tmp/w", "run/t").unwrap();

    db.start_run(run.id, 3).unwrap();
    db.save_run_output(run.id, "live output").unwrap();
    assert_eq!(db.run(run.id).unwrap().output, "live output");
    db.finish_run_with_output(run.id, 1, "failed output", 4)
        .unwrap();
    let finished = db.run(run.id).unwrap();
    assert_eq!(finished.status, RunStatus::Failed);
    assert_eq!(finished.output, "failed output");

    db.queue_run(run.id).unwrap();
    let queued = db.run(run.id).unwrap();
    assert_eq!(queued.status, RunStatus::Queued);
    assert_eq!(queued.attempt, 2);
    assert!(queued.output.is_empty());
    assert_eq!(db.queued_runs().unwrap().len(), 1);

    db.queue_run_with_prompt(run.id, "address review").unwrap();
    assert_eq!(
        db.run(run.id).unwrap().prompt.as_deref(),
        Some("address review")
    );
    db.start_run(run.id, 5).unwrap();
    assert!(db.run(run.id).unwrap().prompt.is_none());
    assert_eq!(db.running_count().unwrap(), 1);
    assert_eq!(db.recover_interrupted_runs(6).unwrap(), 1);
    let recovered = db.run(run.id).unwrap();
    assert_eq!(recovered.status, RunStatus::Failed);
    assert!(recovered.error.unwrap().contains("Retry"));
}

#[test]
fn cascade_delete() {
    let db = db();
    let p = db.create_project("R", "/tmp/r3", "main", 1).unwrap();
    let t = db.create_task(p.id, "T", "p", 1).unwrap();
    db.create_run(t.id, "codex", "/wt/x", "b").unwrap();

    db.delete_project(p.id).unwrap();
    assert!(db.tasks(p.id).unwrap().is_empty());
    assert!(db.runs(t.id).unwrap().is_empty());
}

#[test]
fn annotations_batch_and_resolve() {
    let db = db();
    let p = db.create_project("R", "/tmp/ann", "main", 1).unwrap();
    let t = db.create_task(p.id, "T", "p", 1).unwrap();
    let r = db.create_run(t.id, "codex", "/wt/x", "b").unwrap();

    db.add_annotation(r.id, "src/main.rs", 10, Side::New, "rename this", 5)
        .unwrap();
    let a2 = db
        .add_annotation(r.id, "src/main.rs", 3, Side::New, "extract fn", 6)
        .unwrap();
    // Ordered by file then line: line 3 comes before line 10.
    let anns = db.annotations(r.id).unwrap();
    assert_eq!(anns.len(), 2);
    assert_eq!(anns[0].line, 3);
    assert_eq!(db.open_annotation_count(r.id).unwrap(), 2);

    db.resolve_annotation(a2.id, true).unwrap();
    assert_eq!(db.open_annotation_count(r.id).unwrap(), 1);
}

#[test]
fn accounts_hot_swap_active() {
    let db = db();
    let a = db.add_account("claude", "me@work", 1).unwrap();
    let b = db.add_account("claude", "me@personal", 2).unwrap();
    // First account is active by default.
    assert!(a.active);
    assert!(!b.active);
    assert_eq!(db.active_account("claude").unwrap().unwrap().id, a.id);

    db.activate_account(b.id).unwrap();
    assert_eq!(db.active_account("claude").unwrap().unwrap().id, b.id);
    assert_eq!(db.accounts(Some("claude")).unwrap().len(), 2);
    assert!(db.active_account("codex").unwrap().is_none());
}

#[test]
fn deleting_active_account_promotes_the_next_account() {
    let db = db();
    let first = db.add_account("claude", "work", 1).unwrap();
    let second = db.add_account("claude", "personal", 2).unwrap();
    db.delete_account(first.id).unwrap();
    assert_eq!(db.active_account("claude").unwrap().unwrap().id, second.id);
    assert_eq!(db.accounts(Some("claude")).unwrap().len(), 1);
}

#[test]
fn usage_snapshot_and_fraction() {
    let db = db();
    let a = db.add_account("claude", "me", 1).unwrap();
    db.record_usage(a.id, 250, Some(1000), Some(9999), 5)
        .unwrap();
    let u = db.latest_usage(a.id).unwrap().unwrap();
    assert_eq!(u.used, 250);
    assert_eq!(u.fraction(), Some(0.25));

    // Later snapshot wins.
    db.record_usage(a.id, 900, Some(1000), Some(9999), 10)
        .unwrap();
    assert_eq!(db.latest_usage(a.id).unwrap().unwrap().used, 900);
}

#[test]
fn notifications_unread_flow() {
    let db = db();
    let n1 = db
        .notify("run_finished", "Codex done", "", None, 1)
        .unwrap();
    db.notify("attention", "Needs input", "", None, 2).unwrap();
    assert_eq!(db.unread_count().unwrap(), 2);
    assert_eq!(db.notifications(true).unwrap().len(), 2);
    // Newest first.
    assert_eq!(db.notifications(false).unwrap()[0].title, "Needs input");

    db.mark_read(n1.id, true).unwrap();
    assert_eq!(db.unread_count().unwrap(), 1);
    db.mark_all_read().unwrap();
    assert_eq!(db.unread_count().unwrap(), 0);
}

#[test]
fn pinned_and_recent_ordering() {
    let db = db();
    let a = db.create_project("A", "/tmp/a", "main", 1).unwrap();
    let b = db.create_project("B", "/tmp/b", "main", 2).unwrap();
    let c = db.create_project("C", "/tmp/c", "main", 3).unwrap();

    db.touch_project(a.id, 100).unwrap();
    db.touch_project(b.id, 200).unwrap();
    db.set_pinned(c.id, true).unwrap();

    // Pinned first (C), then by last_opened desc (B, then A).
    let order: Vec<String> = db.projects().unwrap().into_iter().map(|p| p.name).collect();
    assert_eq!(order, vec!["C", "B", "A"]);

    // Recents excludes never-opened C, newest first.
    let recent: Vec<String> = db
        .recent_projects(10)
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(recent, vec!["B", "A"]);
}

#[test]
fn missing_lookups_report_not_found() {
    let db = db();
    assert!(matches!(db.task(999), Err(Error::NotFound)));
    assert!(matches!(db.run(999), Err(Error::NotFound)));
    assert!(matches!(
        db.set_task_status(999, TaskStatus::Merged, 1),
        Err(Error::NotFound)
    ));
}

#[test]
fn note_vault_attachments_and_rename_are_durable() {
    let db = db();
    let project = db.create_project("Notes", "/tmp/notes", "main", 1).unwrap();
    let task = db
        .create_task(project.id, "Fix cache", "cache prompt", 2)
        .unwrap();
    let run = db
        .create_run(task.id, "agent", "/tmp/wt", "task/cache")
        .unwrap();

    db.set_note_vault(project.id, NoteVaultMode::Repository, "/tmp/notes/notes")
        .unwrap();
    assert_eq!(
        db.note_vault(project.id).unwrap().unwrap().mode,
        NoteVaultMode::Repository
    );
    db.attach_note_to_task(project.id, "cache.md", task.id, 3)
        .unwrap();
    db.attach_note_to_run(project.id, "cache.md", run.id, 4)
        .unwrap();
    assert_eq!(db.task_note_paths(task.id).unwrap(), vec!["cache.md"]);
    assert_eq!(db.run_note_paths(run.id).unwrap(), vec!["cache.md"]);
    assert_eq!(
        db.note_attachments(project.id, "cache.md").unwrap().len(),
        2
    );

    db.rename_note_attachments(project.id, "cache.md", "caching.md")
        .unwrap();
    assert_eq!(db.run_note_paths(run.id).unwrap(), vec!["caching.md"]);
    db.delete_note_attachments(project.id, "caching.md")
        .unwrap();
    assert!(db.run_note_paths(run.id).unwrap().is_empty());
}

#[test]
fn unified_store_search_finds_tasks_and_transcripts() {
    let db = db();
    let project = db
        .create_project("Search", "/tmp/search", "main", 1)
        .unwrap();
    let task = db
        .create_task(project.id, "Fix cache", "Investigate stale cache keys", 2)
        .unwrap();
    let run = db
        .create_run(task.id, "agent", "/tmp/wt", "task/cache")
        .unwrap();
    db.save_run_output(run.id, "cache invalidation repaired")
        .unwrap();
    let records = db.search_project(project.id, "cache", 20).unwrap();
    assert_eq!(records.len(), 2);
    assert!(records.iter().any(|record| record.kind == SearchKind::Task));
    assert!(records.iter().any(|record| record.kind == SearchKind::Run));
}
