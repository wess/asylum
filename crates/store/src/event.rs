//! Append-only event log CRUD. Every meaningful state change records an
//! [`Event`]; the companion and control servers replay `events_since` a cursor
//! so a phone or an agent can follow the fleet without polling every table. The
//! log is never mutated after insert.

use rusqlite::{params, Row};

use crate::model::Event;
use crate::{Db, Result};

fn from_row(row: &Row) -> rusqlite::Result<Event> {
    Ok(Event {
        id: row.get("id")?,
        kind: row.get("kind")?,
        task_id: row.get("task_id")?,
        run_id: row.get("run_id")?,
        data: row.get("data")?,
        created_at: row.get("created_at")?,
    })
}

impl Db {
    /// Append an event to the log. `data` is opaque JSON detail.
    pub fn record_event(
        &self,
        kind: &str,
        task_id: Option<i64>,
        run_id: Option<i64>,
        data: &str,
        now: i64,
    ) -> Result<Event> {
        self.conn().execute(
            "INSERT INTO events (kind, task_id, run_id, data, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![kind, task_id, run_id, data, now],
        )?;
        let id = self.conn().last_insert_rowid();
        self.conn()
            .query_row("SELECT * FROM events WHERE id = ?1", params![id], from_row)
            .map_err(Into::into)
    }

    /// Events after `since` (exclusive), oldest first, capped at `limit`. Pass
    /// `since = 0` to start from the beginning; use the last returned id as the
    /// next cursor.
    pub fn events_since(&self, since: i64, limit: i64) -> Result<Vec<Event>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT * FROM events WHERE id > ?1 ORDER BY id ASC LIMIT ?2")?;
        let rows = stmt
            .query_map(params![since, limit.max(0)], from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// The id of the most recent event, or 0 when the log is empty. Lets a
    /// client subscribe to "only what happens from now on".
    pub fn latest_event_id(&self) -> Result<i64> {
        let id: i64 =
            self.conn()
                .query_row("SELECT COALESCE(MAX(id), 0) FROM events", [], |row| {
                    row.get(0)
                })?;
        Ok(id)
    }

    /// Trim the event log to its most recent `keep` rows. Called periodically so
    /// a long-lived session's log stays bounded.
    pub fn prune_events(&self, keep: i64) -> Result<usize> {
        Ok(self.conn().execute(
            "DELETE FROM events WHERE id <= (
                 SELECT COALESCE(MAX(id), 0) - ?1 FROM events
             )",
            params![keep.max(0)],
        )?)
    }
}

#[cfg(test)]
#[path = "../tests/event.rs"]
mod tests;
