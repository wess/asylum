//! Review annotation CRUD - line comments on a run's diff.

use rusqlite::{params, Row};

use crate::model::{Annotation, Side};
use crate::{Db, Error, Result};

fn from_row(row: &Row) -> rusqlite::Result<Annotation> {
    let side: String = row.get("side")?;
    Ok(Annotation {
        id: row.get("id")?,
        run_id: row.get("run_id")?,
        file: row.get("file")?,
        line: row.get::<_, i64>("line")? as u32,
        side: Side::parse(&side),
        body: row.get("body")?,
        resolved: row.get::<_, i64>("resolved")? != 0,
        created_at: row.get("created_at")?,
    })
}

impl Db {
    /// Add a review comment anchored to a line of a run's diff.
    pub fn add_annotation(
        &self,
        run_id: i64,
        file: &str,
        line: u32,
        side: Side,
        body: &str,
        now: i64,
    ) -> Result<Annotation> {
        self.conn().execute(
            "INSERT INTO annotations (run_id, file, line, side, body, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![run_id, file, line as i64, side.as_str(), body, now],
        )?;
        self.annotation(self.conn().last_insert_rowid())
    }

    /// Fetch one annotation.
    pub fn annotation(&self, id: i64) -> Result<Annotation> {
        self.conn()
            .query_row("SELECT * FROM annotations WHERE id = ?1", params![id], from_row)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
                other => other.into(),
            })
    }

    /// All annotations on a run, in file/line order - the batch shipped back to
    /// the agent as review feedback.
    pub fn annotations(&self, run_id: i64) -> Result<Vec<Annotation>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM annotations WHERE run_id = ?1 ORDER BY file, line, id",
        )?;
        let rows = stmt.query_map(params![run_id], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Count unresolved annotations on a run.
    pub fn open_annotation_count(&self, run_id: i64) -> Result<usize> {
        let n: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM annotations WHERE run_id = ?1 AND resolved = 0",
            params![run_id],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// Mark an annotation resolved or reopened.
    pub fn resolve_annotation(&self, id: i64, resolved: bool) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE annotations SET resolved = ?2 WHERE id = ?1",
            params![id, resolved as i64],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Delete an annotation.
    pub fn delete_annotation(&self, id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM annotations WHERE id = ?1", params![id])?;
        Ok(())
    }
}
