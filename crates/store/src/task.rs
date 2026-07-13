//! Task CRUD.

use rusqlite::{params, Row};

use crate::model::{Task, TaskStatus};
use crate::{Db, Error, Result};

fn from_row(row: &Row) -> rusqlite::Result<Task> {
    let status: String = row.get("status")?;
    Ok(Task {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        title: row.get("title")?,
        prompt: row.get("prompt")?,
        status: TaskStatus::parse(&status),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

impl Db {
    /// Create a task in `Draft` status.
    pub fn create_task(
        &self,
        project_id: i64,
        title: &str,
        prompt: &str,
        now: i64,
    ) -> Result<Task> {
        self.conn().execute(
            "INSERT INTO tasks (project_id, title, prompt, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'draft', ?4, ?4)",
            params![project_id, title, prompt, now],
        )?;
        self.task(self.conn().last_insert_rowid())
    }

    /// Fetch a task by id.
    pub fn task(&self, id: i64) -> Result<Task> {
        self.conn()
            .query_row("SELECT * FROM tasks WHERE id = ?1", params![id], from_row)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
                other => other.into(),
            })
    }

    /// All tasks for a project, newest first.
    pub fn tasks(&self, project_id: i64) -> Result<Vec<Task>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM tasks WHERE project_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![project_id], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Move a task to a new status, stamping `updated_at`.
    pub fn set_task_status(&self, id: i64, status: TaskStatus, now: i64) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE tasks SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, status.as_str(), now],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// Delete a task and (via cascade) its runs.
    pub fn delete_task(&self, id: i64) -> Result<()> {
        self.conn()
            .execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        Ok(())
    }
}
