//! Named agents: the roster, and what each one remembers.

use rusqlite::{params, Row};

use crate::model::NamedAgent;
use crate::{Db, Error, Result};

fn from_row(row: &Row) -> rusqlite::Result<NamedAgent> {
    Ok(NamedAgent {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        name: row.get("name")?,
        role: row.get("role")?,
        agent_id: row.get("agent_id")?,
        memory: row.get("memory")?,
        created_at: row.get("created_at")?,
        last_used_at: row.get("last_used_at")?,
    })
}

impl Db {
    /// Hire one, or update the role of an existing one of the same name.
    ///
    /// Memory is deliberately *not* touched on update: changing what somebody
    /// is for should not erase what they have learned, and re-running the
    /// command that created them is the most likely way to lose it.
    pub fn save_named_agent(
        &self,
        project_id: i64,
        name: &str,
        role: &str,
        agent_id: &str,
        now: i64,
    ) -> Result<NamedAgent> {
        self.conn().execute(
            "INSERT INTO named_agents (project_id, name, role, agent_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(project_id, name) DO UPDATE SET
               role = excluded.role,
               agent_id = excluded.agent_id",
            params![project_id, name, role, agent_id, now],
        )?;
        self.named_agent(project_id, name)
    }

    pub fn named_agent(&self, project_id: i64, name: &str) -> Result<NamedAgent> {
        self.conn()
            .query_row(
                "SELECT * FROM named_agents WHERE project_id = ?1 AND name = ?2",
                params![project_id, name],
                from_row,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
                other => other.into(),
            })
    }

    pub fn named_agent_by_id(&self, id: i64) -> Result<NamedAgent> {
        self.conn()
            .query_row(
                "SELECT * FROM named_agents WHERE id = ?1",
                params![id],
                from_row,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound,
                other => other.into(),
            })
    }

    /// The project's roster, most recently used first — the order you think of
    /// colleagues in.
    pub fn named_agents(&self, project_id: i64) -> Result<Vec<NamedAgent>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT * FROM named_agents WHERE project_id = ?1
             ORDER BY last_used_at DESC NULLS LAST, name ASC",
        )?;
        let rows = stmt.query_map(params![project_id], from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Add a line to what an agent knows. `true` if this was new.
    ///
    /// Appends rather than replaces, and refuses to append the same line twice.
    /// An agent that writes "they only sign annual" after every task would
    /// otherwise fill its own memory with one fact until nothing else fits.
    ///
    /// The bool is so a caller can say "already knew that" rather than report
    /// success for a write that did nothing — a memory you cannot tell you
    /// failed to add to is one you stop trusting.
    pub fn remember(&self, id: i64, line: &str) -> Result<bool> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(false);
        }
        let current = self.named_agent_by_id(id)?.memory;
        // Compare the fact, not the rendering: stored lines carry the bullet.
        let known = |stored: &str| stored.trim().trim_start_matches("- ").trim() == line;
        if current.lines().any(known) {
            return Ok(false);
        }
        let next = if current.trim().is_empty() {
            format!("- {line}")
        } else {
            format!("{}\n- {line}", current.trim_end())
        };
        let n = self.conn().execute(
            "UPDATE named_agents SET memory = ?2 WHERE id = ?1",
            params![id, next],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        Ok(true)
    }

    /// Forget everything one agent has learned, keeping the agent.
    pub fn forget(&self, id: i64) -> Result<()> {
        self.conn().execute(
            "UPDATE named_agents SET memory = '' WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn touch_named_agent(&self, id: i64, now: i64) -> Result<()> {
        self.conn().execute(
            "UPDATE named_agents SET last_used_at = ?2 WHERE id = ?1",
            params![id, now],
        )?;
        Ok(())
    }

    /// Record that a run was done by a named agent.
    ///
    /// Separate from `create_run` so the ordinary path — a one-off run with no
    /// identity — does not grow an argument that is `None` at every call site
    /// but one.
    pub fn assign_run_agent(&self, run_id: i64, named_agent_id: i64, now: i64) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE runs SET named_agent_id = ?2 WHERE id = ?1",
            params![run_id, named_agent_id],
        )?;
        if n == 0 {
            return Err(Error::NotFound);
        }
        self.touch_named_agent(named_agent_id, now)
    }

    /// Who did this run, if anybody in particular.
    pub fn run_agent(&self, run_id: i64) -> Result<Option<NamedAgent>> {
        let id: Option<i64> = self.conn().query_row(
            "SELECT named_agent_id FROM runs WHERE id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        match id {
            Some(id) => Ok(self.named_agent_by_id(id).ok()),
            None => Ok(None),
        }
    }

    pub fn delete_named_agent(&self, project_id: i64, name: &str) -> Result<()> {
        self.conn().execute(
            "DELETE FROM named_agents WHERE project_id = ?1 AND name = ?2",
            params![project_id, name],
        )?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/namedagent.rs"]
mod tests;
