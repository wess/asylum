//! Notification CRUD - agent-finished / attention / check-failed events with an
//! unread state ("return to later").

use rusqlite::{params, Row};

use crate::model::Notification;
use crate::{Db, Error, Result};

fn from_row(row: &Row) -> rusqlite::Result<Notification> {
    Ok(Notification {
        id: row.get("id")?,
        kind: row.get("kind")?,
        title: row.get("title")?,
        body: row.get("body")?,
        run_id: row.get("run_id")?,
        read: row.get::<_, i64>("read")? != 0,
        created_at: row.get("created_at")?,
    })
}

impl Db {
    /// Post a notification.
    pub fn notify(
        &self,
        kind: &str,
        title: &str,
        body: &str,
        run_id: Option<i64>,
        now: i64,
    ) -> Result<Notification> {
        self.conn().execute(
            "INSERT INTO notifications (kind, title, body, run_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![kind, title, body, run_id, now],
        )?;
        let id = self.conn().last_insert_rowid();
        self.conn()
            .query_row("SELECT * FROM notifications WHERE id = ?1", params![id], from_row)
            .map_err(Into::into)
    }

    /// Notifications newest-first. With `unread_only`, filters to unread.
    pub fn notifications(&self, unread_only: bool) -> Result<Vec<Notification>> {
        let conn = self.conn();
        let sql = if unread_only {
            "SELECT * FROM notifications WHERE read = 0 ORDER BY created_at DESC, id DESC"
        } else {
            "SELECT * FROM notifications ORDER BY created_at DESC, id DESC"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Count unread notifications (the badge number).
    pub fn unread_count(&self) -> Result<usize> {
        let n: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM notifications WHERE read = 0",
            [],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// Mark one notification read (or unread - "return to later").
    pub fn mark_read(&self, id: i64, read: bool) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE notifications SET read = ?2 WHERE id = ?1",
            params![id, read as i64],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Mark every notification read.
    pub fn mark_all_read(&self) -> Result<()> {
        self.conn()
            .execute("UPDATE notifications SET read = 1 WHERE read = 0", [])?;
        Ok(())
    }
}
