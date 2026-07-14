//! Search task prompts and persisted run transcripts within one project.

use rusqlite::params;

use crate::{Db, Result, SearchKind, SearchRecord};

impl Db {
    pub fn search_project(
        &self,
        project_id: i64,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchRecord>> {
        let mut statement = self.conn().prepare(
            "SELECT kind, id, title, detail FROM (
                SELECT 'task' AS kind, tasks.id AS id, tasks.title AS title,
                       substr(tasks.prompt, 1, 400) AS detail, tasks.updated_at AS stamp
                FROM tasks
                WHERE tasks.project_id = ?1
                  AND (?2 = '' OR instr(lower(tasks.title), lower(?2)) > 0
                       OR instr(lower(tasks.prompt), lower(?2)) > 0)
                UNION ALL
                SELECT 'run' AS kind, runs.id AS id,
                       'Run #' || runs.id || ': ' || runs.agent AS title,
                       substr(CASE WHEN runs.error IS NOT NULL THEN runs.error ELSE runs.output END, 1, 400) AS detail,
                       coalesce(runs.ended_at, runs.started_at, tasks.updated_at) AS stamp
                FROM runs JOIN tasks ON tasks.id = runs.task_id
                WHERE tasks.project_id = ?1
                  AND (?2 = '' OR instr(lower(runs.agent), lower(?2)) > 0
                       OR instr(lower(runs.branch), lower(?2)) > 0
                       OR instr(lower(runs.output), lower(?2)) > 0
                       OR instr(lower(coalesce(runs.error, '')), lower(?2)) > 0)
             ) ORDER BY stamp DESC LIMIT ?3",
        )?;
        let rows = statement.query_map(params![project_id, query.trim(), limit as i64], |row| {
            let kind: String = row.get("kind")?;
            Ok(SearchRecord {
                kind: if kind == "run" {
                    SearchKind::Run
                } else {
                    SearchKind::Task
                },
                id: row.get("id")?,
                title: row.get("title")?,
                detail: row.get("detail")?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
