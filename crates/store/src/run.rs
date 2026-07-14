//! Run CRUD - one agent's attempt at a task in its own worktree.

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
        output: row.get("output")?,
        error: row.get("error")?,
        attempt: row.get("attempt")?,
        prompt: row.get("prompt")?,
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
        let mut stmt = conn.prepare("SELECT * FROM runs WHERE task_id = ?1 ORDER BY id ASC")?;
        let rows = stmt.query_map(params![task_id], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Mark a run started (status → `Running`) at `now`.
    pub fn start_run(&self, id: i64, now: i64) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE runs SET status = 'running', started_at = ?2,
                    ended_at = NULL, exit_code = NULL, error = NULL, prompt = NULL
             WHERE id = ?1",
            params![id, now],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
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

    /// Finish a run and persist its latest terminal transcript.
    pub fn finish_run_with_output(
        &self,
        id: i64,
        exit_code: i32,
        output: &str,
        now: i64,
    ) -> Result<()> {
        let status = if exit_code == 0 {
            "succeeded"
        } else {
            "failed"
        };
        let n = self.conn().execute(
            "UPDATE runs SET status = ?2, ended_at = ?3, exit_code = ?4,
                    output = ?5, error = NULL WHERE id = ?1",
            params![id, status, now, exit_code, output],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Mark a launch/runtime failure while retaining any transcript captured.
    pub fn fail_run(&self, id: i64, message: &str, output: &str, now: i64) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE runs SET status = 'failed', ended_at = ?2, exit_code = NULL,
                    output = ?3, error = ?4 WHERE id = ?1",
            params![id, now, output, message],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Cancel a run.
    pub fn cancel_run(&self, id: i64, now: i64) -> Result<()> {
        self.update_status(id, RunStatus::Cancelled, None, Some(now), None)
    }

    /// Cancel a live run and preserve the transcript visible at cancellation.
    pub fn cancel_run_with_output(&self, id: i64, output: &str, now: i64) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE runs SET status = 'cancelled', ended_at = ?2,
                    output = ?3 WHERE id = ?1",
            params![id, now, output],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Persist a live transcript snapshot without changing lifecycle state.
    pub fn save_run_output(&self, id: i64, output: &str) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE runs SET output = ?2 WHERE id = ?1",
            params![id, output],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Queue an existing worktree for another attempt.
    pub fn queue_run(&self, id: i64) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE runs SET status = 'queued', started_at = NULL, ended_at = NULL,
                    exit_code = NULL, output = '', error = NULL, prompt = NULL,
                    attempt = attempt + 1 WHERE id = ?1",
            params![id],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Queue another attempt in the same worktree with a durable follow-up.
    pub fn queue_run_with_prompt(&self, id: i64, prompt: &str) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE runs SET status = 'queued', started_at = NULL, ended_at = NULL,
                    exit_code = NULL, output = '', error = NULL, prompt = ?2,
                    attempt = attempt + 1 WHERE id = ?1",
            params![id, prompt],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Runs waiting to launch, globally ordered by creation id.
    pub fn queued_runs(&self) -> Result<Vec<Run>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT * FROM runs WHERE status = 'queued' ORDER BY id ASC")?;
        let rows = stmt.query_map([], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Number of processes the store currently considers live.
    pub fn running_count(&self) -> Result<usize> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM runs WHERE status = 'running'",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Agent ids that have completed at least one real run successfully.
    pub fn successful_agents(&self) -> Result<Vec<String>> {
        let conn = self.conn();
        let mut statement = conn
            .prepare("SELECT DISTINCT agent FROM runs WHERE status = 'succeeded' ORDER BY agent")?;
        let rows = statement.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// A prior app process cannot still own a pty after restart. Reconcile
    /// those rows as failed instead of leaving permanently-running cards.
    pub fn recover_interrupted_runs(&self, now: i64) -> Result<usize> {
        Ok(self.conn().execute(
            "UPDATE runs SET status = 'failed', ended_at = ?1,
                    error = 'The app closed before this run finished. Retry to continue in the same worktree.'
             WHERE status = 'running'",
            params![now],
        )?)
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
