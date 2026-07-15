//! Connection setup and idempotent migrations.
//!
//! Migrations are an ordered list of SQL steps guarded by `user_version`. On
//! open we read the pragma, apply every step past it, and bump the version.
//! Add a migration by appending to [`MIGRATIONS`] - never edit an existing one.

use rusqlite::Connection;

/// Ordered schema steps. The index+1 is the `user_version` after applying it.
const MIGRATIONS: &[&str] = &[
    // 1 - core entities.
    "CREATE TABLE projects (
        id          INTEGER PRIMARY KEY,
        name        TEXT NOT NULL,
        path        TEXT NOT NULL UNIQUE,
        base_branch TEXT NOT NULL DEFAULT 'main',
        created_at  INTEGER NOT NULL
    );
    CREATE TABLE tasks (
        id         INTEGER PRIMARY KEY,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        title      TEXT NOT NULL,
        prompt     TEXT NOT NULL,
        status     TEXT NOT NULL DEFAULT 'draft',
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX idx_tasks_project ON tasks(project_id);
    CREATE TABLE runs (
        id         INTEGER PRIMARY KEY,
        task_id    INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        agent      TEXT NOT NULL,
        worktree   TEXT NOT NULL,
        branch     TEXT NOT NULL,
        status     TEXT NOT NULL DEFAULT 'queued',
        started_at INTEGER,
        ended_at   INTEGER,
        exit_code  INTEGER
    );
    CREATE INDEX idx_runs_task ON runs(task_id);",
    // 2 - review annotations, provider accounts + usage, notifications, and
    // project pin/recency.
    "CREATE TABLE annotations (
        id         INTEGER PRIMARY KEY,
        run_id     INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
        file       TEXT NOT NULL,
        line       INTEGER NOT NULL,
        side       TEXT NOT NULL DEFAULT 'new',
        body       TEXT NOT NULL,
        resolved   INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX idx_annotations_run ON annotations(run_id);
    CREATE TABLE accounts (
        id         INTEGER PRIMARY KEY,
        provider   TEXT NOT NULL,
        label      TEXT NOT NULL,
        active     INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX idx_accounts_provider ON accounts(provider);
    CREATE TABLE usage (
        id          INTEGER PRIMARY KEY,
        account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        used        INTEGER NOT NULL,
        limit_      INTEGER,
        resets_at   INTEGER,
        captured_at INTEGER NOT NULL
    );
    CREATE INDEX idx_usage_account ON usage(account_id);
    CREATE TABLE notifications (
        id         INTEGER PRIMARY KEY,
        kind       TEXT NOT NULL,
        title      TEXT NOT NULL,
        body       TEXT NOT NULL DEFAULT '',
        run_id     INTEGER REFERENCES runs(id) ON DELETE SET NULL,
        read       INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );
    ALTER TABLE projects ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE projects ADD COLUMN last_opened_at INTEGER NOT NULL DEFAULT 0;",
    // 3 - durable run diagnostics and terminal output. Output stays on the run
    // so a finished or interrupted session remains reviewable after restart.
    "ALTER TABLE runs ADD COLUMN output TEXT NOT NULL DEFAULT '';
    ALTER TABLE runs ADD COLUMN error TEXT;
    ALTER TABLE runs ADD COLUMN attempt INTEGER NOT NULL DEFAULT 1;",
    // 4 - verification results belong to one run/worktree, not the task UI.
    "CREATE TABLE runchecks (
        run_id      INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
        id          TEXT NOT NULL,
        status      TEXT NOT NULL,
        summary     TEXT NOT NULL DEFAULT '',
        duration_ms INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (run_id, id)
    );",
    // 5 - queued review continuations must survive an app restart.
    "ALTER TABLE runs ADD COLUMN prompt TEXT;",
    // 6 - Markdown vault placement and durable note context links.
    "CREATE TABLE notevaults (
        project_id INTEGER PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
        mode       TEXT NOT NULL,
        path       TEXT NOT NULL
    );
    CREATE TABLE noteattachments (
        id         INTEGER PRIMARY KEY,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        note_path  TEXT NOT NULL,
        task_id    INTEGER REFERENCES tasks(id) ON DELETE CASCADE,
        run_id     INTEGER REFERENCES runs(id) ON DELETE CASCADE,
        created_at INTEGER NOT NULL,
        CHECK ((task_id IS NULL) != (run_id IS NULL)),
        UNIQUE (note_path, task_id),
        UNIQUE (note_path, run_id)
    );
    CREATE INDEX idx_noteattachments_project ON noteattachments(project_id);
    CREATE INDEX idx_noteattachments_task ON noteattachments(task_id);
    CREATE INDEX idx_noteattachments_run ON noteattachments(run_id);",
    // 7 - follow-ups queued from the mobile companion. The desktop app drains
    // pending rows and delivers each to an active run of the task, so a phone
    // message actually reaches the agent instead of only landing in the inbox.
    "CREATE TABLE followups (
        id         INTEGER PRIMARY KEY,
        task_id    INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        message    TEXT NOT NULL,
        source     TEXT NOT NULL DEFAULT 'companion',
        processed  INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX idx_followups_pending ON followups(processed);",
    // 8 - live semantic activity on a run (idle/working/blocked/done), the
    // agent control-request queue (an in-worktree agent asks the app to spawn a
    // helper run, run checks, …; drained like followups), and an append-only
    // event log powering the companion and control event streams.
    "ALTER TABLE runs ADD COLUMN activity TEXT;
    CREATE TABLE controlrequests (
        id         INTEGER PRIMARY KEY,
        task_id    INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        run_id     INTEGER REFERENCES runs(id) ON DELETE SET NULL,
        kind       TEXT NOT NULL,
        payload    TEXT NOT NULL DEFAULT '',
        source     TEXT NOT NULL DEFAULT 'agent',
        processed  INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX idx_controlrequests_pending ON controlrequests(processed);
    CREATE TABLE events (
        id         INTEGER PRIMARY KEY,
        kind       TEXT NOT NULL,
        task_id    INTEGER,
        run_id     INTEGER,
        data       TEXT NOT NULL DEFAULT '',
        created_at INTEGER NOT NULL
    );",
    // 9 - retryable, auditable drain queues. The boolean `processed` flag hid
    // failures (a row was marked done even when delivery/execution failed) and
    // could not be retried. Replace it with an explicit lifecycle
    // (pending/running/succeeded/failed) plus attempt count, last error, claim
    // and completion timestamps, and a backoff schedule, on both the followups
    // and controlrequests queues.
    "DROP INDEX idx_followups_pending;
    ALTER TABLE followups ADD COLUMN status TEXT NOT NULL DEFAULT 'pending';
    ALTER TABLE followups ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE followups ADD COLUMN last_error TEXT;
    ALTER TABLE followups ADD COLUMN claimed_at INTEGER;
    ALTER TABLE followups ADD COLUMN completed_at INTEGER;
    ALTER TABLE followups ADD COLUMN next_attempt_at INTEGER NOT NULL DEFAULT 0;
    UPDATE followups SET status = 'succeeded', attempts = 1 WHERE processed = 1;
    ALTER TABLE followups DROP COLUMN processed;
    CREATE INDEX idx_followups_pending ON followups(status, next_attempt_at);

    DROP INDEX idx_controlrequests_pending;
    ALTER TABLE controlrequests ADD COLUMN status TEXT NOT NULL DEFAULT 'pending';
    ALTER TABLE controlrequests ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE controlrequests ADD COLUMN last_error TEXT;
    ALTER TABLE controlrequests ADD COLUMN claimed_at INTEGER;
    ALTER TABLE controlrequests ADD COLUMN completed_at INTEGER;
    ALTER TABLE controlrequests ADD COLUMN next_attempt_at INTEGER NOT NULL DEFAULT 0;
    UPDATE controlrequests SET status = 'succeeded', attempts = 1 WHERE processed = 1;
    ALTER TABLE controlrequests DROP COLUMN processed;
    CREATE INDEX idx_controlrequests_pending ON controlrequests(status, next_attempt_at);",
];

/// Apply pragmas and any pending migrations.
pub(crate) fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    // `journal_mode`/`foreign_keys` must be set outside any transaction.
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;",
    )?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, step) in MIGRATIONS.iter().enumerate() {
        let target = (i + 1) as i64;
        if version < target {
            apply_migration(conn, step, target)?;
        }
    }
    Ok(())
}

/// Apply one migration `step` and its `user_version` bump as a single
/// transaction. On any failure the transaction is rolled back, so a crash or
/// error mid-migration leaves the database at the *previous* complete schema
/// version rather than a half-applied step that would fail every future open.
fn apply_migration(conn: &Connection, step: &str, target: i64) -> rusqlite::Result<()> {
    conn.execute_batch("BEGIN")?;
    // user_version can't be parameterized; `target` is a trusted index+1.
    let applied = conn
        .execute_batch(step)
        .and_then(|_| conn.execute_batch(&format!("PRAGMA user_version = {target}")));
    match applied {
        Ok(()) => conn.execute_batch("COMMIT"),
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

#[cfg(test)]
#[path = "../tests/schema.rs"]
mod tests;
