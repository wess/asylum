//! Schedules: work that starts without anybody present.
//!
//! The store half only. Deciding *when* something is due is arithmetic and
//! lives here, tested; actually fanning it out belongs to the app, which owns
//! worktrees and agents.

use rusqlite::{params, Row};

use crate::model::Schedule;
use crate::{Db, Error, Result};

fn from_row(row: &Row) -> rusqlite::Result<Schedule> {
    Ok(Schedule {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        title: row.get("title")?,
        prompt: row.get("prompt")?,
        agents: row.get("agents")?,
        every_minutes: row.get("every_minutes")?,
        enabled: row.get::<_, i64>("enabled")? != 0,
        next_at: row.get("next_at")?,
        last_run_at: row.get("last_run_at")?,
        created_at: row.get("created_at")?,
    })
}

/// When a schedule with this cadence should next fire, given that it just did.
///
/// Catches up rather than replaying. An app closed over a long weekend comes
/// back to a schedule that missed three nights, and the useful behaviour is one
/// run now and the cadence resumed — not three runs at once, each fanning out
/// to several agents, for work that has since been done twice over.
///
/// The returned time is always in the future, which is what stops a due
/// schedule being picked up again on the very next tick.
pub fn advance(previous_next: i64, every_minutes: i64, now: i64) -> i64 {
    let step = every_minutes.max(1) * 60;
    let mut next = previous_next + step;
    if next <= now {
        // Skip whole missed periods in one jump rather than looping once per
        // period, so a schedule dormant for a year does not spin here.
        let behind = now - next;
        next += ((behind / step) + 1) * step;
    }
    next
}

/// What to create a schedule from.
///
/// A struct rather than seven positional arguments: `create_schedule(p, a, b,
/// "", 1440, t, n)` is a line nobody can read, and two of those numbers are
/// times that would swap silently.
#[derive(Debug, Clone)]
pub struct NewSchedule<'a> {
    pub project_id: i64,
    pub title: &'a str,
    pub prompt: &'a str,
    /// Comma-separated agent ids; empty means the project's defaults.
    pub agents: &'a str,
    pub every_minutes: i64,
    /// When it should first come due.
    pub first_at: i64,
}

impl Db {
    pub fn create_schedule(&self, new: NewSchedule<'_>, now: i64) -> Result<Schedule> {
        self.conn().execute(
            "INSERT INTO schedules
               (project_id, title, prompt, agents, every_minutes, next_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                new.project_id,
                new.title,
                new.prompt,
                new.agents,
                new.every_minutes.max(1),
                new.first_at,
                now
            ],
        )?;
        self.schedule(self.conn().last_insert_rowid())
    }

    pub fn schedule(&self, id: i64) -> Result<Schedule> {
        self.conn()
            .query_row(
                "SELECT * FROM schedules WHERE id = ?1",
                params![id],
                from_row,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
                other => other.into(),
            })
    }

    /// Every schedule for a project, soonest first.
    pub fn schedules(&self, project_id: i64) -> Result<Vec<Schedule>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT * FROM schedules WHERE project_id = ?1 ORDER BY next_at ASC")?;
        let rows = stmt.query_map(params![project_id], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Schedules that are enabled and due at `now`, across every project.
    pub fn due_schedules(&self, now: i64) -> Result<Vec<Schedule>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM schedules WHERE enabled = 1 AND next_at <= ?1 ORDER BY next_at ASC",
        )?;
        let rows = stmt.query_map(params![now], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Record that a schedule fired and move it on.
    ///
    /// Called *before* the work is dispatched, not after. A fan-out takes time
    /// and can fail, and a schedule that only advances on success fires again
    /// on the next tick — turning one broken run into a machine that starts a
    /// new one every fifteen seconds.
    pub fn mark_schedule_fired(&self, id: i64, now: i64) -> Result<()> {
        let schedule = self.schedule(id)?;
        let next = advance(schedule.next_at, schedule.every_minutes, now);
        let n = self.conn().execute(
            "UPDATE schedules SET next_at = ?2, last_run_at = ?3 WHERE id = ?1",
            params![id, next, now],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub fn set_schedule_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE schedules SET enabled = ?2 WHERE id = ?1",
            params![id, enabled as i64],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    pub fn delete_schedule(&self, id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM schedules WHERE id = ?1", params![id])?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/schedule.rs"]
mod tests;
