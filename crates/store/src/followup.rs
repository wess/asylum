//! Follow-up queue CRUD and delivery lifecycle. Rows are enqueued by the
//! companion server and drained by the desktop app, which delivers each message
//! to an active run. Delivery is *claimed* before it runs and its outcome is
//! recorded, so a failure is never silently represented as success and a crash
//! mid-delivery is recovered rather than lost. See [`crate::queue`].

use rusqlite::{params, Row};

use crate::model::{Followup, QueueStatus};
use crate::queue;
use crate::{Db, Result};

fn from_row(row: &Row) -> rusqlite::Result<Followup> {
    Ok(Followup {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        message: row.get("message")?,
        source: row.get("source")?,
        status: QueueStatus::parse(&row.get::<_, String>("status")?),
        attempts: row.get("attempts")?,
        last_error: row.get("last_error")?,
        created_at: row.get("created_at")?,
    })
}

impl Db {
    /// Queue a follow-up message against a task.
    pub fn queue_followup(
        &self,
        task_id: i64,
        message: &str,
        source: &str,
        now: i64,
    ) -> Result<Followup> {
        self.conn().execute(
            "INSERT INTO followups (task_id, message, source, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![task_id, message, source, now],
        )?;
        self.followup(self.conn().last_insert_rowid())
    }

    /// Fetch one follow-up by id.
    pub fn followup(&self, id: i64) -> Result<Followup> {
        self.conn()
            .query_row(
                "SELECT * FROM followups WHERE id = ?1",
                params![id],
                from_row,
            )
            .map_err(Into::into)
    }

    /// Pending follow-ups (awaiting a first or retried delivery), oldest first.
    /// A backed-off retry is included even if its delay has not elapsed; use
    /// [`Db::claim_followups`] to take only those ready to run now.
    pub fn pending_followups(&self) -> Result<Vec<Followup>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT * FROM followups WHERE status = 'pending' ORDER BY created_at, id")?;
        let rows = stmt
            .query_map([], from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Atomically claim every pending follow-up whose backoff has elapsed:
    /// mark it `running`, bump its attempt count, and return the claimed rows
    /// (oldest first). A single UPDATE ... RETURNING makes the claim indivisible,
    /// so a second drain call cannot re-claim the same work.
    pub fn claim_followups(&self, now: i64) -> Result<Vec<Followup>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "UPDATE followups
             SET status = 'running', attempts = attempts + 1, claimed_at = ?1
             WHERE status = 'pending' AND next_attempt_at <= ?1
             RETURNING *",
        )?;
        let mut rows = stmt
            .query_map(params![now], from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        // RETURNING order is unspecified; deliver oldest first.
        rows.sort_by_key(|r| (r.created_at, r.id));
        Ok(rows)
    }

    /// Record a claimed follow-up as delivered.
    pub fn complete_followup(&self, id: i64, now: i64) -> Result<()> {
        self.conn().execute(
            "UPDATE followups
             SET status = 'succeeded', completed_at = ?2, last_error = NULL
             WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    /// Record a transient delivery failure. Returns the follow-up to `pending`
    /// with a backoff until its attempts are exhausted, after which it becomes a
    /// terminal `failed` row preserved for inspection. Returns whether it will be
    /// retried.
    pub fn fail_followup(&self, id: i64, now: i64, error: &str) -> Result<bool> {
        queue_fail(self, "followups", id, now, error, false)
    }

    /// Record a permanent delivery failure (bad request, missing target): mark
    /// `failed` immediately without further retries.
    pub fn fail_followup_permanent(&self, id: i64, now: i64, error: &str) -> Result<()> {
        queue_fail(self, "followups", id, now, error, true).map(|_| ())
    }

    /// Recover follow-ups stranded `running` by a crash: return them to
    /// `pending` (or `failed` if attempts are spent) so they are retried rather
    /// than lost. Returns how many rows were recovered.
    pub fn recover_stale_followups(&self, now: i64) -> Result<usize> {
        queue_recover(self, "followups", now)
    }
}

/// Shared failure transition for a claimed queue row, by table name. When
/// `permanent` (or attempts are exhausted) the row becomes terminal `failed`;
/// otherwise it returns to `pending` with a backoff. Returns whether it will be
/// retried.
pub(crate) fn queue_fail(
    db: &Db,
    table: &str,
    id: i64,
    now: i64,
    error: &str,
    permanent: bool,
) -> Result<bool> {
    let attempts: i64 = db.conn().query_row(
        &format!("SELECT attempts FROM {table} WHERE id = ?1"),
        params![id],
        |r| r.get(0),
    )?;
    let terminal = permanent || attempts >= queue::MAX_ATTEMPTS;
    if terminal {
        db.conn().execute(
            &format!(
                "UPDATE {table}
                 SET status = 'failed', last_error = ?2, completed_at = ?3, claimed_at = NULL
                 WHERE id = ?1"
            ),
            params![id, error, now],
        )?;
    } else {
        let retry_at = now.saturating_add(queue::backoff_secs(attempts));
        db.conn().execute(
            &format!(
                "UPDATE {table}
                 SET status = 'pending', last_error = ?2, next_attempt_at = ?3, claimed_at = NULL
                 WHERE id = ?1"
            ),
            params![id, error, retry_at],
        )?;
    }
    Ok(!terminal)
}

/// Shared crash-recovery transition, by table name.
pub(crate) fn queue_recover(db: &Db, table: &str, now: i64) -> Result<usize> {
    let stale_before = now.saturating_sub(queue::STALE_RUNNING_SECS);
    let n = db.conn().execute(
        &format!(
            "UPDATE {table}
             SET status = CASE WHEN attempts >= ?2 THEN 'failed' ELSE 'pending' END,
                 claimed_at = NULL,
                 last_error = COALESCE(last_error, 'interrupted before completion'),
                 completed_at = CASE WHEN attempts >= ?2 THEN ?3 ELSE completed_at END
             WHERE status = 'running' AND (claimed_at IS NULL OR claimed_at <= ?1)"
        ),
        params![stale_before, queue::MAX_ATTEMPTS, now],
    )?;
    Ok(n)
}

#[cfg(test)]
#[path = "../tests/followup.rs"]
mod tests;
