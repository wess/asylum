//! Routines: a workflow shown once and replayed thereafter.
//!
//! The store half. What a step *is* — a shell command, recorded from a session
//! somebody actually worked through — is the CLI's business; here it is an
//! ordered list with a name, scoped to a project because "the release steps"
//! means something different in each repository.

use rusqlite::{params, Row};

use crate::model::Routine;
use crate::{Db, Error, Result};

fn from_row(row: &Row) -> rusqlite::Result<Routine> {
    Ok(Routine {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        steps: row.get("steps")?,
        created_at: row.get("created_at")?,
        last_run_at: row.get("last_run_at")?,
    })
}

impl Db {
    /// Save a routine, replacing one of the same name in the same project.
    ///
    /// Replacing rather than refusing: recording is how you fix a routine that
    /// was wrong, and making somebody delete the old one first turns "show it
    /// again" into two steps for no benefit.
    pub fn save_routine(
        &self,
        project_id: i64,
        name: &str,
        description: &str,
        steps: &[String],
        now: i64,
    ) -> Result<Routine> {
        let json = serde_json::to_string(steps).unwrap_or_else(|_| "[]".to_string());
        self.conn().execute(
            "INSERT INTO routines (project_id, name, description, steps, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(project_id, name) DO UPDATE SET
               description = excluded.description,
               steps = excluded.steps",
            params![project_id, name, description, json, now],
        )?;
        self.routine(project_id, name)
    }

    pub fn routine(&self, project_id: i64, name: &str) -> Result<Routine> {
        self.conn()
            .query_row(
                "SELECT * FROM routines WHERE project_id = ?1 AND name = ?2",
                params![project_id, name],
                from_row,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
                other => other.into(),
            })
    }

    pub fn routines(&self, project_id: i64) -> Result<Vec<Routine>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT * FROM routines WHERE project_id = ?1 ORDER BY name ASC")?;
        let rows = stmt.query_map(params![project_id], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn mark_routine_run(&self, id: i64, now: i64) -> Result<()> {
        self.conn().execute(
            "UPDATE routines SET last_run_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    pub fn delete_routine(&self, project_id: i64, name: &str) -> Result<()> {
        self.conn().execute(
            "DELETE FROM routines WHERE project_id = ?1 AND name = ?2",
            params![project_id, name],
        )?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/routine.rs"]
mod tests;
