//! Per-project note vault settings and task/run attachments.

use rusqlite::{params, Row};

use crate::{Db, NoteAttachment, NoteVault, NoteVaultMode, Result};

fn attachment(row: &Row) -> rusqlite::Result<NoteAttachment> {
    Ok(NoteAttachment {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        note_path: row.get("note_path")?,
        task_id: row.get("task_id")?,
        run_id: row.get("run_id")?,
        created_at: row.get("created_at")?,
    })
}

impl Db {
    pub fn note_vault(&self, project_id: i64) -> Result<Option<NoteVault>> {
        let mut statement = self
            .conn()
            .prepare("SELECT project_id, mode, path FROM notevaults WHERE project_id = ?1")?;
        let mut rows = statement.query(params![project_id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let mode: String = row.get("mode")?;
        Ok(Some(NoteVault {
            project_id: row.get("project_id")?,
            mode: NoteVaultMode::parse(&mode),
            path: row.get("path")?,
        }))
    }

    pub fn set_note_vault(
        &self,
        project_id: i64,
        mode: NoteVaultMode,
        path: &str,
    ) -> Result<NoteVault> {
        self.conn().execute(
            "INSERT INTO notevaults (project_id, mode, path) VALUES (?1, ?2, ?3)
             ON CONFLICT(project_id) DO UPDATE SET mode = excluded.mode, path = excluded.path",
            params![project_id, mode.as_str(), path],
        )?;
        Ok(self.note_vault(project_id)?.expect("upserted note vault"))
    }

    pub fn attach_note_to_task(
        &self,
        project_id: i64,
        note_path: &str,
        task_id: i64,
        created_at: i64,
    ) -> Result<()> {
        self.conn().execute(
            "INSERT OR IGNORE INTO noteattachments
             (project_id, note_path, task_id, run_id, created_at)
             VALUES (?1, ?2, ?3, NULL, ?4)",
            params![project_id, note_path, task_id, created_at],
        )?;
        Ok(())
    }

    pub fn attach_note_to_run(
        &self,
        project_id: i64,
        note_path: &str,
        run_id: i64,
        created_at: i64,
    ) -> Result<()> {
        self.conn().execute(
            "INSERT OR IGNORE INTO noteattachments
             (project_id, note_path, task_id, run_id, created_at)
             VALUES (?1, ?2, NULL, ?3, ?4)",
            params![project_id, note_path, run_id, created_at],
        )?;
        Ok(())
    }

    pub fn note_attachments(
        &self,
        project_id: i64,
        note_path: &str,
    ) -> Result<Vec<NoteAttachment>> {
        self.attachments(
            "SELECT * FROM noteattachments WHERE project_id = ?1 AND note_path = ?2 ORDER BY created_at",
            params![project_id, note_path],
        )
    }

    pub fn task_note_paths(&self, task_id: i64) -> Result<Vec<String>> {
        self.paths(
            "SELECT note_path FROM noteattachments WHERE task_id = ?1 ORDER BY created_at",
            task_id,
        )
    }

    pub fn run_note_paths(&self, run_id: i64) -> Result<Vec<String>> {
        self.paths(
            "SELECT note_path FROM noteattachments WHERE run_id = ?1 ORDER BY created_at",
            run_id,
        )
    }

    pub fn rename_note_attachments(
        &self,
        project_id: i64,
        old_path: &str,
        new_path: &str,
    ) -> Result<()> {
        self.conn().execute(
            "UPDATE noteattachments SET note_path = ?3 WHERE project_id = ?1 AND note_path = ?2",
            params![project_id, old_path, new_path],
        )?;
        Ok(())
    }

    pub fn delete_note_attachments(&self, project_id: i64, note_path: &str) -> Result<()> {
        self.conn().execute(
            "DELETE FROM noteattachments WHERE project_id = ?1 AND note_path = ?2",
            params![project_id, note_path],
        )?;
        Ok(())
    }

    fn paths(&self, sql: &str, id: i64) -> Result<Vec<String>> {
        let mut statement = self.conn().prepare(sql)?;
        let rows = statement.query_map(params![id], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    fn attachments<P>(&self, sql: &str, params: P) -> Result<Vec<NoteAttachment>>
    where
        P: rusqlite::Params,
    {
        let mut statement = self.conn().prepare(sql)?;
        let rows = statement.query_map(params, attachment)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
