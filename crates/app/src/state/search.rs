//! Debounced, cancellable background search across the project's sources,
//! notes, and store records.

use gpui::Context;

use crate::state::Root;

/// One result in the project-wide search surface.
#[derive(Clone)]
pub enum SearchResult {
    File(search::Match),
    Note(notes::Hit),
    Record(store::SearchRecord),
}

/// Debounce window after the last keystroke before a search actually runs, so a
/// burst of typing collapses into a single query.
const SEARCH_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(180);

/// Everything a background search needs, snapshotted from [`Root`] on the UI
/// thread so the heavy work runs on a worker without borrowing app state.
struct SearchInputs {
    /// On-disk store path; a fresh read-only handle is opened against it.
    db_path: std::path::PathBuf,
    project_id: i64,
    project_path: std::path::PathBuf,
    /// The Markdown vault root, if the project has one.
    vault_root: Option<std::path::PathBuf>,
    query: String,
}

/// The product of one background search: the results and an optional error to
/// surface (source search can fail when no ripgrep/git backend is installed).
struct SearchOutcome {
    results: Vec<SearchResult>,
    error: Option<String>,
}

impl SearchInputs {
    /// Run the whole search off the UI thread: store records over a read-only
    /// connection (WAL concurrent read), the Markdown vault, and source files.
    fn run(self) -> SearchOutcome {
        let mut results = Vec::new();
        let mut error = None;

        // Task prompts + run transcripts, via a separate read-only handle so the
        // app's writable Db (owned by the UI thread) is never touched from here.
        if let Ok(db) = store::Db::open_readonly(&self.db_path) {
            if let Ok(records) = db.search_project(self.project_id, &self.query, 120) {
                results.extend(records.into_iter().map(SearchResult::Record));
            }
        }

        // The Markdown note vault. The notes crate stays pure and synchronous;
        // only this call site moved off the UI thread.
        if let Some(root) = &self.vault_root {
            if let Ok(index) = notes::index(root) {
                results.extend(
                    notes::search(&index, &self.query)
                        .into_iter()
                        .take(120)
                        .map(SearchResult::Note),
                );
            }
        }

        // Source files (ripgrep / git grep). Skipped for an empty query, as
        // before - browsing recent tasks/runs/notes needs no file scan.
        if !self.query.is_empty() {
            let options = search::Options {
                fixed: true,
                max_results: 200,
                ..Default::default()
            };
            match search::search(&self.project_path, &self.query, &options) {
                Ok(matches) => results.extend(matches.into_iter().map(SearchResult::File)),
                Err(err) => error = Some(err.to_string()),
            }
        }

        SearchOutcome { results, error }
    }
}

impl Root {
    /// Search source files, notes, task prompts, and persisted transcripts on the
    /// background executor - debounced and cancellable.
    ///
    /// The store is single-threaded and lives here on the UI thread, so rather
    /// than make it async, the heavy work runs on a worker against a *separate
    /// read-only connection* ([`store::Db::open_readonly`]; WAL lets readers run
    /// concurrently with the app's writer). Every call bumps `search_generation`;
    /// a task older than the current generation - superseded by a newer keystroke
    /// during the debounce window, or after a newer result already landed -
    /// discards itself, so a stale query can never overwrite a newer one's
    /// results. Nothing blocks the UI thread and only the last keystroke in a
    /// burst actually runs, so typing stays responsive over a large corpus. The
    /// existing results stay on screen until the new ones replace them (no
    /// mid-type empty flash).
    pub fn run_search(&mut self, cx: &mut Context<Self>) {
        let generation = self.search_generation.wrapping_add(1);
        self.search_generation = generation;

        let Some(pid) = self.project_id else {
            self.search_results.clear();
            cx.notify();
            return;
        };
        let Ok(project) = self.db.project(pid) else {
            return;
        };
        let inputs = SearchInputs {
            db_path: Self::db_path(),
            project_id: pid,
            project_path: std::path::PathBuf::from(project.path),
            vault_root: self
                .db
                .note_vault(pid)
                .ok()
                .flatten()
                .map(|vault| std::path::PathBuf::from(vault.path)),
            query: self.search_query.trim().to_string(),
        };

        let executor = cx.background_executor().clone();
        cx.spawn(async move |handle, cx| {
            // Debounce: collapse a burst of keystrokes into one search. A newer
            // keystroke bumps the generation and supersedes this task.
            executor.timer(SEARCH_DEBOUNCE).await;
            if handle
                .update(cx, |root, _| root.search_generation != generation)
                .unwrap_or(true)
            {
                return;
            }
            let outcome = executor.spawn(async move { inputs.run() }).await;
            handle
                .update(cx, |root, cx| {
                    // Apply only if we are still the newest query.
                    if root.search_generation == generation {
                        root.apply_search(outcome, cx);
                    }
                })
                .ok();
        })
        .detach();
    }

    /// Install a completed search's results (only ever called for the newest
    /// query - the generation guard is checked at the call site).
    fn apply_search(&mut self, outcome: SearchOutcome, cx: &mut Context<Self>) {
        self.search_results = outcome.results;
        if let Some(error) = outcome.error {
            self.push_error("Search failed", error);
        }
        cx.notify();
    }
}
