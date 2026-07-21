//! Project-scoped search over task prompts and persisted run transcripts.
//!
//! # Full-text vs. substring
//!
//! The bundled SQLite is compiled with FTS5 (`ENABLE_FTS5` appears in
//! `pragma_compile_options` - the tests assert this), so migration 10 keeps an
//! external-content FTS5 index over task prompts (`tasks_fts`) and run
//! transcripts (`runs_fts`), maintained by triggers on the base tables. A
//! word/prefix query runs against that index rather than scanning every row's
//! text.
//!
//! FTS5's query grammar is not free-form text: quotes, `*`, `:`, parentheses and
//! the `AND`/`OR`/`NOT`/`NEAR` keywords are operators, and code-ish needles like
//! `foo::bar` or `src/main.rs` are punctuation-heavy. So [`Db::search_project`]
//! dispatches:
//!
//! - a query of only letters/digits/whitespace -> FTS5, each whitespace token
//!   lowercased into a prefix term joined by implicit AND
//!   (`deploy pipe` -> `deploy* pipe*`);
//! - anything containing punctuation -> a `LIKE`/`instr` substring scan, which
//!   matches the literal needle (right for code-ish queries) and can never form
//!   a malformed `MATCH` expression;
//! - an empty query -> the most recent tasks and runs.
//!
//! # Threading
//!
//! The primary [`Db`] is single-threaded and lives on the app's UI thread.
//! Searches run on a background executor against a separate read-only handle
//! ([`Db::open_readonly`]); WAL mode lets those readers run concurrently with the
//! writer, so nothing here blocks the UI thread.
//!
//! # Input caps
//!
//! Query length, per-term length, term count, and result count are all bounded
//! ([`MAX_QUERY_CHARS`], [`MAX_TERM_CHARS`], [`MAX_TERMS`], [`MAX_RESULTS`]) so a
//! pathological input can neither build an absurd query nor return an unbounded
//! result set.

use rusqlite::params;

use crate::{Db, Result, SearchKind, SearchRecord};

/// Longest query honored; extra characters are dropped before matching.
const MAX_QUERY_CHARS: usize = 256;
/// Longest single FTS prefix term.
const MAX_TERM_CHARS: usize = 64;
/// Most AND-ed terms in one FTS query.
const MAX_TERMS: usize = 16;
/// Hard ceiling on returned rows, regardless of the caller's `limit`.
const MAX_RESULTS: usize = 500;

/// The task half of the result projection, scoped to one project (`?1`).
const TASK_ROWS: &str = "SELECT 'task' AS kind, tasks.id AS id, tasks.title AS title, \
     substr(tasks.prompt, 1, 400) AS detail, tasks.updated_at AS stamp \
     FROM tasks WHERE tasks.project_id = ?1";

/// The run half of the result projection, scoped to one project (`?1`).
const RUN_ROWS: &str = "SELECT 'run' AS kind, runs.id AS id, \
     'Run #' || runs.id || ': ' || runs.agent AS title, \
     substr(CASE WHEN runs.error IS NOT NULL THEN runs.error ELSE runs.output END, 1, 400) AS detail, \
     coalesce(runs.ended_at, runs.started_at, tasks.updated_at) AS stamp \
     FROM runs JOIN tasks ON tasks.id = runs.task_id WHERE tasks.project_id = ?1";

impl Db {
    /// Search a project's task prompts and run transcripts for `query`, newest
    /// first, at most `limit` (capped at [`MAX_RESULTS`]) rows.
    pub fn search_project(
        &self,
        project_id: i64,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchRecord>> {
        let limit = limit.min(MAX_RESULTS) as i64;
        let query = capped_query(query);
        if query.is_empty() {
            return self.search_recent(project_id, limit);
        }
        match fts_query(&query) {
            Some(fts) => self.search_fts(project_id, &fts, limit),
            None => self.search_like(project_id, &query, limit),
        }
    }

    /// The most recent tasks and runs, for an empty query (the browse view).
    fn search_recent(&self, project_id: i64, limit: i64) -> Result<Vec<SearchRecord>> {
        let sql = format!(
            "SELECT kind, id, title, detail FROM ({TASK_ROWS} UNION ALL {RUN_ROWS}) \
             ORDER BY stamp DESC LIMIT ?2"
        );
        let mut statement = self.conn().prepare(&sql)?;
        let rows = statement.query_map(params![project_id, limit], map_record)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Full-text search via the FTS5 indexes. `fts` is a validated `MATCH`
    /// expression built by [`fts_query`], reused for both indexes.
    fn search_fts(&self, project_id: i64, fts: &str, limit: i64) -> Result<Vec<SearchRecord>> {
        let sql = format!(
            "SELECT kind, id, title, detail FROM (\
                 {TASK_ROWS} AND tasks.id IN (SELECT rowid FROM tasks_fts WHERE tasks_fts MATCH ?2) \
                 UNION ALL \
                 {RUN_ROWS} AND runs.id IN (SELECT rowid FROM runs_fts WHERE runs_fts MATCH ?2)\
             ) ORDER BY stamp DESC LIMIT ?3"
        );
        let mut statement = self.conn().prepare(&sql)?;
        let rows = statement.query_map(params![project_id, fts, limit], map_record)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Literal-substring fallback for punctuated queries FTS5 can't parse.
    fn search_like(&self, project_id: i64, query: &str, limit: i64) -> Result<Vec<SearchRecord>> {
        let sql = format!(
            "SELECT kind, id, title, detail FROM (\
                 {TASK_ROWS} AND (instr(lower(tasks.title), lower(?2)) > 0 \
                     OR instr(lower(tasks.prompt), lower(?2)) > 0) \
                 UNION ALL \
                 {RUN_ROWS} AND (instr(lower(runs.agent), lower(?2)) > 0 \
                     OR instr(lower(runs.branch), lower(?2)) > 0 \
                     OR instr(lower(runs.output), lower(?2)) > 0 \
                     OR instr(lower(coalesce(runs.error, '')), lower(?2)) > 0)\
             ) ORDER BY stamp DESC LIMIT ?3"
        );
        let mut statement = self.conn().prepare(&sql)?;
        let rows = statement.query_map(params![project_id, query, limit], map_record)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

/// Map a projected row to a [`SearchRecord`].
fn map_record(row: &rusqlite::Row) -> rusqlite::Result<SearchRecord> {
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
}

/// Trim and cap the raw query length so a pathological input is bounded before
/// it ever reaches the query builders.
fn capped_query(query: &str) -> String {
    query.trim().chars().take(MAX_QUERY_CHARS).collect()
}

/// Build an FTS5 `MATCH` expression from a trimmed query, or `None` when the
/// query should take the substring fallback instead.
///
/// Only queries made purely of letters/digits/whitespace are FTS-eligible; any
/// punctuation (`:`, `/`, `-`, quotes, operators, …) hands off to `LIKE`, which
/// matches the literal needle. Eligible queries become an implicit-AND of
/// lowercased prefix terms, bounded by [`MAX_TERMS`] and [`MAX_TERM_CHARS`].
fn fts_query(query: &str) -> Option<String> {
    if !query
        .chars()
        .all(|c| c.is_alphanumeric() || c.is_whitespace())
    {
        return None;
    }
    let terms: Vec<String> = query
        .split_whitespace()
        .take(MAX_TERMS)
        .map(|token| {
            let token: String = token
                .chars()
                .take(MAX_TERM_CHARS)
                .flat_map(char::to_lowercase)
                .collect();
            format!("{token}*")
        })
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

#[cfg(test)]
#[path = "../tests/search.rs"]
mod tests;
