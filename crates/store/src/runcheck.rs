//! Durable verification results for one run worktree.

use rusqlite::{params, Row};

use crate::{Db, Result, RunCheck};

fn from_row(row: &Row) -> rusqlite::Result<RunCheck> {
    Ok(RunCheck {
        run_id: row.get("run_id")?,
        id: row.get("id")?,
        status: row.get("status")?,
        summary: row.get("summary")?,
        duration_ms: row.get("duration_ms")?,
    })
}

impl Db {
    pub fn replace_run_checks(&self, run_id: i64, checks: &[RunCheck]) -> Result<()> {
        let transaction = self.conn().unchecked_transaction()?;
        transaction.execute("DELETE FROM runchecks WHERE run_id = ?1", params![run_id])?;
        let mut statement = transaction.prepare(
            "INSERT INTO runchecks (run_id, id, status, summary, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for check in checks {
            statement.execute(params![
                run_id,
                check.id,
                check.status,
                check.summary,
                check.duration_ms,
            ])?;
        }
        drop(statement);
        transaction.commit()?;
        Ok(())
    }

    pub fn run_checks(&self, run_id: i64) -> Result<Vec<RunCheck>> {
        let conn = self.conn();
        let mut statement =
            conn.prepare("SELECT * FROM runchecks WHERE run_id = ?1 ORDER BY id ASC")?;
        let rows = statement.query_map(params![run_id], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

#[cfg(test)]
#[path = "../tests/runcheck.rs"]
mod tests;
