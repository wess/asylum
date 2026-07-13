//! Connection setup and idempotent migrations.
//!
//! Migrations are an ordered list of SQL steps guarded by `user_version`. On
//! open we read the pragma, apply every step past it, and bump the version.
//! Add a migration by appending to [`MIGRATIONS`] — never edit an existing one.

use rusqlite::Connection;

/// Ordered schema steps. The index+1 is the `user_version` after applying it.
const MIGRATIONS: &[&str] = &[
    // 1 — core entities.
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
    // 2 — review annotations, provider accounts + usage, notifications, and
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
];

/// Apply pragmas and any pending migrations.
pub(crate) fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;",
    )?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, step) in MIGRATIONS.iter().enumerate() {
        let target = (i + 1) as i64;
        if version < target {
            conn.execute_batch(step)?;
            // user_version can't be parameterized; the value is a trusted usize.
            conn.execute_batch(&format!("PRAGMA user_version = {target}"))?;
        }
    }
    Ok(())
}
