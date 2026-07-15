//! Control-request queue CRUD and execution lifecycle. Rows are enqueued by the
//! control surface (an in-worktree agent, or the CLI) and drained by the desktop
//! app, which performs the git/pty side-effects the request asks for - spawning
//! a helper run, running checks, and so on. Mirrors the
//! [`followup`](crate::followup) queue: work is claimed before it runs, its
//! outcome is recorded, and a crash mid-execution is recovered.

use rusqlite::{params, Row};

use crate::followup::{queue_fail, queue_recover};
use crate::model::{ControlRequest, QueueStatus};
use crate::{Db, Error, Result};

fn from_row(row: &Row) -> rusqlite::Result<ControlRequest> {
    Ok(ControlRequest {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        run_id: row.get("run_id")?,
        kind: row.get("kind")?,
        payload: row.get("payload")?,
        source: row.get("source")?,
        status: QueueStatus::parse(&row.get::<_, String>("status")?),
        attempts: row.get("attempts")?,
        last_error: row.get("last_error")?,
        created_at: row.get("created_at")?,
    })
}

impl Db {
    /// Queue a control request against a task. `run_id` is the issuing run when
    /// the request came from inside a worktree. `payload` is opaque JSON the app
    /// interprets per `kind`.
    pub fn queue_control_request(
        &self,
        task_id: i64,
        run_id: Option<i64>,
        kind: &str,
        payload: &str,
        source: &str,
        now: i64,
    ) -> Result<ControlRequest> {
        self.conn().execute(
            "INSERT INTO controlrequests (task_id, run_id, kind, payload, source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![task_id, run_id, kind, payload, source, now],
        )?;
        self.control_request(self.conn().last_insert_rowid())
    }

    /// Fetch one control request by id.
    pub fn control_request(&self, id: i64) -> Result<ControlRequest> {
        self.conn()
            .query_row(
                "SELECT * FROM controlrequests WHERE id = ?1",
                params![id],
                from_row,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
                other => other.into(),
            })
    }

    /// Pending control requests (awaiting a first or retried run), oldest first.
    /// Use [`Db::claim_control_requests`] to take only those ready to run now.
    pub fn pending_control_requests(&self) -> Result<Vec<ControlRequest>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM controlrequests WHERE status = 'pending' ORDER BY created_at, id",
        )?;
        let rows = stmt
            .query_map([], from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Atomically claim every pending control request whose backoff has elapsed:
    /// mark it `running`, bump its attempt count, and return the claimed rows
    /// (oldest first). A single UPDATE ... RETURNING makes the claim indivisible,
    /// so a second drain call cannot re-claim the same work.
    pub fn claim_control_requests(&self, now: i64) -> Result<Vec<ControlRequest>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "UPDATE controlrequests
             SET status = 'running', attempts = attempts + 1, claimed_at = ?1
             WHERE status = 'pending' AND next_attempt_at <= ?1
             RETURNING *",
        )?;
        let mut rows = stmt
            .query_map(params![now], from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.sort_by_key(|r| (r.created_at, r.id));
        Ok(rows)
    }

    /// Record a claimed control request as executed.
    pub fn complete_control_request(&self, id: i64, now: i64) -> Result<()> {
        self.conn().execute(
            "UPDATE controlrequests
             SET status = 'succeeded', completed_at = ?2, last_error = NULL
             WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    /// Record a transient execution failure: return to `pending` with a backoff
    /// until attempts are exhausted, then terminal `failed`. Returns whether it
    /// will be retried.
    pub fn fail_control_request(&self, id: i64, now: i64, error: &str) -> Result<bool> {
        queue_fail(self, "controlrequests", id, now, error, false)
    }

    /// Record a permanent execution failure (bad payload, unknown agent/kind):
    /// mark `failed` immediately without further retries.
    pub fn fail_control_request_permanent(&self, id: i64, now: i64, error: &str) -> Result<()> {
        queue_fail(self, "controlrequests", id, now, error, true).map(|_| ())
    }

    /// Recover control requests stranded `running` by a crash: return them to
    /// `pending` (or `failed` if attempts are spent). Returns how many rows were
    /// recovered.
    pub fn recover_stale_control_requests(&self, now: i64) -> Result<usize> {
        queue_recover(self, "controlrequests", now)
    }
}

#[cfg(test)]
#[path = "../tests/control.rs"]
mod tests;
