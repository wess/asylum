//! Application state: the single [`Root`] entity owns the [`store::Db`] and the
//! current selection. Render reads plain snapshots ([`RunRow`], [`TreeProject`]
//! etc.) out of the store so the view code never holds a database borrow
//! across a closure.

mod accounts;
mod design;
mod inbox;
mod integrations;
mod plugins;
mod projects;
mod review;
mod search;
mod views;

pub use self::search::SearchResult;
pub use accounts::AccountRow;
pub use projects::{TreeProject, TreeRun, TreeTask};
pub use review::RunRow;
pub use views::{more_rail, View};

use libsinclair::termview::TermView;
use store::Db;

/// A clock helper - unix seconds. Kept in one place so the rest of the app never
/// touches `SystemTime` directly.
pub fn now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The root application state.
pub struct Root {
    pub db: Db,
    pub project_id: Option<i64>,
    pub task_id: Option<i64>,
    /// The run all review, terminal, and merge actions currently target.
    pub selected_run_id: Option<i64>,
    /// Agent ids selected for the next fan-out.
    pub fanout: Vec<String>,
    /// Live search query and its results (Search view).
    pub search_query: String,
    pub search_results: Vec<SearchResult>,
    pub search_input: Option<gpui::Entity<guise::TextInput>>,
    /// Monotonic search generation. Each `run_search` bumps it; a background
    /// search whose generation is stale (superseded by a newer keystroke)
    /// discards its result instead of overwriting a newer query's.
    pub search_generation: u64,
    /// Per-project Markdown vault and editor state.
    pub note: crate::note::State,
    /// GitHub PRs/issues for the Integrations view (loaded on demand).
    pub prs: Vec<github::PullRequest>,
    pub issues: Vec<github::Issue>,
    /// Linear issues for the Integrations view (loaded on demand when a token
    /// is configured).
    pub linear_issues: Vec<linear::Issue>,
    /// Last integration load error, if any.
    pub integration_error: Option<String>,
    /// The file last opened in an editor tab (tracked so Preview can follow it).
    pub editor_file: Option<String>,
    /// Run ids whose worktree checks are executing on the background executor.
    pub checking_runs: std::collections::HashSet<i64>,
    /// The design-mode element capture awaiting a note (click → note → pin).
    pub pending_capture: Option<designmode::Capture>,
    /// Design annotations collected for the next "send to agent".
    pub design_annotations: Vec<designmode::Annotation>,
    /// The design-note input (built lazily), for the browser/preview toolbar.
    pub design_note: Option<gpui::Entity<guise::TextInput>>,
    /// Web views (by entity id) with design mode switched on.
    pub design_enabled: std::collections::HashSet<gpui::EntityId>,
    /// The diff line the next review comment anchors to (file, line, side).
    pub review_target: Option<(String, u32, store::Side)>,
    /// The command palette and quick-open overlays (built lazily).
    pub palette: Option<gpui::Entity<guise::overlay::Spotlight>>,
    pub quickopen: Option<gpui::Entity<guise::overlay::Spotlight>>,
    /// The review-comment input (built lazily), for diff annotations.
    pub review_note: Option<gpui::Entity<guise::TextInput>>,
    /// Expanded node ids in the workspace tree (`project-<id>`, `task-<id>`).
    pub expanded: std::collections::HashSet<String>,
    /// The right-click context menu (rebuilt per invocation).
    pub context_menu: Option<gpui::Entity<guise::overlay::ContextMenu>>,
    /// The new-task prompt input on the Tasks board (built lazily).
    pub compose: Option<gpui::Entity<guise::TextInput>>,
    /// The ref new worktrees start from (branch or commit). Empty = the
    /// project's base branch.
    pub start_ref: String,
    /// The start-from-ref input in the advanced compose controls.
    pub start_ref_input: Option<gpui::Entity<guise::TextInput>>,
    /// The "add account" input on the Accounts surface (`provider: label`).
    pub account_input: Option<gpui::Entity<guise::TextInput>>,
    /// Diff review layout: false = unified, true = side-by-side (old | new).
    pub diff_split: bool,
    /// Live pty views keyed by their durable run row.
    pub run_terms: std::collections::HashMap<i64, gpui::Entity<TermView>>,
    /// Pidfiles naming the live agents' process groups, captured at spawn, so
    /// ending a run signals the group directly instead of trusting entity-drop
    /// timing (see `reap`). In-memory only: a stale file left by a crashed
    /// session is never consulted.
    pub run_pidfiles: std::collections::HashMap<i64, std::path::PathBuf>,
    /// Last unix second a live terminal was snapshotted to SQLite.
    pub run_saved_at: std::collections::HashMap<i64, i64>,
    /// Prevent repeated transcript persistence errors from flooding notices.
    pub run_save_failed: std::collections::HashSet<i64>,
    /// An exit or settings change asks the next frame to fill queue capacity.
    pub launch_needed: bool,
    /// Worktrees and project setup commands are being prepared off the UI thread.
    pub fanout_in_progress: bool,
    /// Set while a fan-out is preparing; flipping it cancels the in-flight setup
    /// (kills the running command's process group and stops the rest). Shared
    /// with the background prepare job.
    pub fanout_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// Actionable messages shown above the current surface.
    pub notices: Vec<crate::run::Notice>,
    pub next_notice_id: u64,
    /// Destructive actions wait here for explicit confirmation.
    pub confirm: Option<crate::run::ConfirmAction>,
    /// A non-git folder selected during onboarding, pending user consent.
    pub pending_project: Option<std::path::PathBuf>,
    /// Show in-app agent settings before the first repository is opened.
    pub onboarding_settings: bool,
    /// Progressive disclosure controls for the task composer.
    pub composer_advanced: bool,
    pub show_all_agents: bool,
    /// Whether the left navigation is reduced to its icon rail.
    pub sidebar_collapsed: bool,
    /// Width of the expanded left navigation, in px (drag the divider to resize).
    pub sidebar_width: f32,
    pub setup_open: bool,
    pub setup_checks: Vec<crate::setup::Check>,
    /// The tabbed, splittable main-area layout.
    pub workspace: crate::workspace::Workspace,
    /// Monotonic id source for tabs.
    pub next_tab_id: u64,
    /// The resolved settings (settings.json layered over defaults), kept
    /// current by the live-reload watcher.
    pub settings: config::Settings,
    /// Problems from the last settings load, surfaced on the Settings tab.
    pub settings_diagnostics: Vec<config::Diagnostic>,
    /// Keeps the settings.json watcher thread alive (drop = stop watching).
    pub settings_watch: Option<config::WatchHandle>,
    /// The Settings surface's text inputs (built lazily).
    pub settings_inputs: Option<crate::settings::Inputs>,
    /// Which Settings accordion sections are collapsed (by section key). All but
    /// the first start collapsed so the page opens compact.
    pub settings_collapsed: std::collections::HashSet<&'static str>,
    /// Live per-agent CLI probes, keyed by agent id, shown on the agent's row.
    pub agent_tests: std::collections::HashMap<String, crate::settings::Test>,
}

impl Root {
    /// The on-disk store path: `$XDG_DATA_HOME/asylum/workspace.sqlite` (or
    /// `~/.local/share/asylum/...`). Shared with the companion server.
    pub fn db_path() -> std::path::PathBuf {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| {
                std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
            })
            .unwrap_or_else(|| std::path::PathBuf::from(".local/share"));
        base.join("asylum").join("workspace.sqlite")
    }

    /// Open the on-disk store and select the most-recently-opened project (if
    /// any). No demo data - the app starts empty and onboards via "Open project".
    pub fn seeded() -> Self {
        let path = Self::db_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let (db, boot_error) = match Db::open(&path) {
            Ok(db) => (db, None),
            Err(error) => (
                Db::memory().expect("open fallback store"),
                Some(format!(
                    "Could not open {}: {error}. This session is temporary; fix the path before doing important work.",
                    path.display()
                )),
            ),
        };
        let recovered = db.recover_interrupted_runs(now()).unwrap_or(0);
        let project_id = db.projects().ok().and_then(|p| p.first().map(|p| p.id));
        let task_id = project_id
            .and_then(|pid| db.tasks(pid).ok())
            .and_then(|t| t.first().map(|t| t.id));
        let selected_run_id = task_id
            .and_then(|tid| db.runs(tid).ok())
            .and_then(|runs| runs.first().map(|run| run.id));
        let mut notices = Vec::new();
        if let Some(message) = boot_error {
            notices.push(crate::run::Notice::error(1, "Store unavailable", message));
        }
        if recovered > 0 {
            notices.push(crate::run::Notice::warning(
                2,
                "Interrupted runs recovered",
                format!("{recovered} run(s) were marked failed. Retry them to continue in their existing worktrees."),
            ));
        }
        let setup_open = db.successful_agents().unwrap_or_default().is_empty();
        let mut root = Root {
            db,
            project_id,
            task_id,
            selected_run_id,
            fanout: Vec::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            search_input: None,
            search_generation: 0,
            note: crate::note::State::default(),
            prs: Vec::new(),
            issues: Vec::new(),
            linear_issues: Vec::new(),
            integration_error: None,
            editor_file: None,
            checking_runs: std::collections::HashSet::new(),
            pending_capture: None,
            design_annotations: Vec::new(),
            design_note: None,
            design_enabled: std::collections::HashSet::new(),
            review_target: None,
            palette: None,
            quickopen: None,
            review_note: None,
            expanded: std::collections::HashSet::new(),
            context_menu: None,
            compose: None,
            start_ref: String::new(),
            start_ref_input: None,
            account_input: None,
            diff_split: false,
            run_terms: std::collections::HashMap::new(),
            run_pidfiles: std::collections::HashMap::new(),
            run_saved_at: std::collections::HashMap::new(),
            run_save_failed: std::collections::HashSet::new(),
            launch_needed: true,
            fanout_in_progress: false,
            fanout_cancel: None,
            next_notice_id: notices.len() as u64 + 1,
            notices,
            confirm: None,
            pending_project: None,
            onboarding_settings: false,
            composer_advanced: false,
            show_all_agents: false,
            sidebar_collapsed: false,
            sidebar_width: 280.0,
            setup_open,
            setup_checks: Vec::new(),
            workspace: crate::workspace::Workspace::new(0),
            next_tab_id: 1,
            settings: config::Settings::default(),
            settings_diagnostics: Vec::new(),
            settings_watch: None,
            settings_inputs: None,
            settings_collapsed: crate::settings::default_collapsed(),
            agent_tests: std::collections::HashMap::new(),
        };
        root.refresh_setup();
        root
    }

    /// A fresh, monotonic tab id.
    pub fn next_tab_id(&mut self) -> u64 {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        id
    }
}

#[cfg(test)]
#[path = "../tests/state.rs"]
mod tests;
