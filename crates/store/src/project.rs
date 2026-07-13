//! Project CRUD.

use rusqlite::{params, Row};

use crate::model::Project;
use crate::{Db, Error, Result};

fn from_row(row: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get("id")?,
        name: row.get("name")?,
        path: row.get("path")?,
        base_branch: row.get("base_branch")?,
        created_at: row.get("created_at")?,
        pinned: row.get::<_, i64>("pinned")? != 0,
        last_opened_at: row.get("last_opened_at")?,
    })
}

impl Db {
    /// Insert a project (or return the existing one with the same path). Returns
    /// the stored row including its assigned id.
    pub fn create_project(
        &self,
        name: &str,
        path: &str,
        base_branch: &str,
        now: i64,
    ) -> Result<Project> {
        if let Some(existing) = self.project_by_path(path)? {
            return Ok(existing);
        }
        self.conn().execute(
            "INSERT INTO projects (name, path, base_branch, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![name, path, base_branch, now],
        )?;
        let id = self.conn().last_insert_rowid();
        self.project(id)
    }

    /// Fetch a project by id.
    pub fn project(&self, id: i64) -> Result<Project> {
        self.conn()
            .query_row(
                "SELECT * FROM projects WHERE id = ?1",
                params![id],
                from_row,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
                other => other.into(),
            })
    }

    /// Fetch a project by its filesystem path, if present.
    pub fn project_by_path(&self, path: &str) -> Result<Option<Project>> {
        match self.conn().query_row(
            "SELECT * FROM projects WHERE path = ?1",
            params![path],
            from_row,
        ) {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// All projects, pinned first, then most-recently-opened (workspace order).
    pub fn projects(&self) -> Result<Vec<Project>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM projects
             ORDER BY pinned DESC, last_opened_at DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// The `limit` most-recently-opened projects (recent repositories).
    pub fn recent_projects(&self, limit: usize) -> Result<Vec<Project>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM projects WHERE last_opened_at > 0
             ORDER BY last_opened_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Pin or unpin a project.
    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE projects SET pinned = ?2 WHERE id = ?1",
            params![id, pinned as i64],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Stamp a project as opened now (updates the recents ordering).
    pub fn touch_project(&self, id: i64, now: i64) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE projects SET last_opened_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Delete a project and (via cascade) its tasks and runs.
    pub fn delete_project(&self, id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        Ok(())
    }
}
