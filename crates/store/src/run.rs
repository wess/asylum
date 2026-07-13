//! Run CRUD — one agent's attempt at a task in its own worktree.

use rusqlite::{params, Row};

use crate::model::{Run, RunStatus};
use crate::{Db, Error, Result};

fn from_row(row: &Row) -> rusqlite::Result<Run> {
    let status: String = row.get("status")?;
    Ok(Run {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        agent: row.get("agent")?,
        worktree: row.get("worktree")?,
        branch: row.get("branch")?,
        status: RunStatus::parse(&status),
        started_at: row.get("started_at")?,
        ended_at: row.get("ended_at")?,
        exit_code: row.get("exit_code")?,
    })
}

impl Db {
    /// Create a run in `Queued` status with its worktree already allocated.
    pub fn create_run(
        &self,
        task_id: i64,
        agent: &str,
        worktree: &str,
        branch: &str,
    ) -> Result<Run> {
        self.conn().execute(
            "INSERT INTO runs (task_id, agent, worktree, branch, status)
             VALUES (?1, ?2, ?3, ?4, 'queued')",
            params![task_id, agent, worktree, branch],
        )?;
        self.run(self.conn().last_insert_rowid())
    }

    /// Fetch a run by id.
    pub fn run(&self, id: i64) -> Result<Run> {
        self.conn()
            .query_row("SELECT * FROM runs WHERE id = ?1", params![id], from_row)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
                other => other.into(),
            })
    }

    /// All runs for a task, oldest first (fan-out order).
    pub fn runs(&self, task_id: i64) -> Result<Vec<Run>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT * FROM runs WHERE task_id = ?1 ORDER BY id ASC")?;
        let rows = stmt.query_map(params![task_id], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Mark a run started (status → `Running`) at `now`.
    pub fn start_run(&self, id: i64, now: i64) -> Result<()> {
        self.update_status(id, RunStatus::Running, Some(now), None, None)
    }

    /// Mark a run finished, recording its terminal status, end time, and exit
    /// code. A zero code maps to `Succeeded`, non-zero to `Failed`, unless a
    /// terminal `status` is passed explicitly (e.g. `Cancelled`).
    pub fn finish_run(&self, id: i64, exit_code: i32, now: i64) -> Result<()> {
        let status = if exit_code == 0 {
            RunStatus::Succeeded
        } else {
            RunStatus::Failed
        };
        self.update_status(id, status, None, Some(now), Some(exit_code))
    }

    /// Cancel a run.
    pub fn cancel_run(&self, id: i64, now: i64) -> Result<()> {
        self.update_status(id, RunStatus::Cancelled, None, Some(now), None)
    }

    /// Shared status writer. `None` fields are left untouched via COALESCE.
    fn update_status(
        &self,
        id: i64,
        status: RunStatus,
        started_at: Option<i64>,
        ended_at: Option<i64>,
        exit_code: Option<i32>,
    ) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE runs SET
                status     = ?2,
                started_at = COALESCE(?3, started_at),
                ended_at   = COALESCE(?4, ended_at),
                exit_code  = COALESCE(?5, exit_code)
             WHERE id = ?1",
            params![id, status.as_str(), started_at, ended_at, exit_code],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }
}
