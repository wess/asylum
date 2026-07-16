//! Application state: the single [`Root`] entity owns the [`store::Db`] and the
//! current selection. Render reads plain snapshots ([`ProjectRow`] etc.) out of
//! the store so the view code never holds a database borrow across a closure.

use gpui::prelude::*;
use gpui::{Context, Focusable, Window};
use libsinclair::termview::{TermOptions, TermView};

use crate::workspace::TabKind;
use store::{Account, Db, Notification, RunStatus, TaskStatus, Usage};

/// Which primary surface the main area shows. The activity bar switches these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// The fan-out board of per-agent run cards.
    Tasks,
    /// Annotatable diff review for the selected run.
    Diff,
    /// Cross-worktree content search.
    Search,
    /// Project Markdown knowledge, links, and task/run context.
    Notes,
    /// GitHub / Linear browsers.
    Integrations,
    /// Provider accounts + usage.
    Accounts,
    /// Notification inbox.
    Notifications,
    /// An embedded terminal in the selected project.
    Terminal,
    /// The built-in code editor with a file tree.
    Editor,
    /// Rich file preview (markdown rendered in a web view).
    Preview,
    /// Embedded browser (design-mode surface).
    Browser,
    /// Installed plugins.
    Plugins,
    /// The settings editor (writes back to settings.json).
    Settings,
}

impl View {
    /// The activity-bar entries in display order: (view, glyph, label).
    pub const BAR: &'static [(View, &'static str, &'static str)] = &[
        (View::Tasks, "◱", "Tasks"),
        (View::Diff, "⌥", "Diff"),
        (View::Search, "⌕", "Search"),
        (View::Notes, "▤", "Notes"),
        (View::Integrations, "◈", "Integrations"),
        (View::Terminal, "▮", "Terminal"),
        (View::Editor, "✎", "Editor"),
        (View::Preview, "▤", "Preview"),
        (View::Browser, "◎", "Browser"),
        (View::Plugins, "⧉", "Plugins"),
        (View::Accounts, "◍", "Accounts"),
        (View::Notifications, "◔", "Inbox"),
    ];

    /// The Lucide icon name for this view.
    pub fn icon(self) -> &'static str {
        match self {
            View::Tasks => "list-todo",
            View::Diff => "git-compare",
            View::Search => "search",
            View::Notes => "file-pen",
            View::Integrations => "github",
            View::Terminal => "terminal",
            View::Editor => "file-pen",
            View::Preview => "eye",
            View::Browser => "globe",
            View::Plugins => "puzzle",
            View::Accounts => "circle-user",
            View::Notifications => "inbox",
            View::Settings => "settings",
        }
    }

    /// The label for this view.
    pub fn label(self) -> &'static str {
        match self {
            View::Tasks => "Tasks",
            View::Diff => "Diff",
            View::Search => "Search",
            View::Notes => "Notes",
            View::Integrations => "Integrations",
            View::Terminal => "Terminal",
            View::Editor => "Editor",
            View::Preview => "Preview",
            View::Browser => "Browser",
            View::Plugins => "Plugins",
            View::Accounts => "Accounts",
            View::Notifications => "Inbox",
            View::Settings => "Settings",
        }
    }
}

/// A clock helper - unix seconds. Kept in one place so the rest of the app never
/// touches `SystemTime` directly.
pub fn now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A git branch name derived from a Linear issue: its identifier plus a short
/// slug of the title (e.g. `eng-123-add-login`).
fn linear_branch(issue: &linear::Issue) -> String {
    let ident = issue.identifier.to_lowercase().replace(['/', ' '], "-");
    let title: String = issue
        .title
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let title = title
        .split('-')
        .filter(|part| !part.is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join("-");
    if title.is_empty() {
        ident
    } else {
        format!("{ident}-{title}")
    }
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
    /// Last unix second a live terminal was snapshotted to SQLite.
    pub run_saved_at: std::collections::HashMap<i64, i64>,
    /// Prevent repeated transcript persistence errors from flooding notices.
    pub run_save_failed: std::collections::HashSet<i64>,
    /// An exit or settings change asks the next frame to fill queue capacity.
    pub launch_needed: bool,
    /// Worktrees and project setup commands are being prepared off the UI thread.
    pub fanout_in_progress: bool,
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
}

/// A project node in the workspace tree.
#[derive(Clone)]
pub struct TreeProject {
    pub id: i64,
    pub name: String,
    pub pinned: bool,
    pub expanded: bool,
    pub tasks: Vec<TreeTask>,
}

/// A task node in the workspace tree.
#[derive(Clone)]
pub struct TreeTask {
    pub id: i64,
    pub title: String,
    pub status: TaskStatus,
    pub expanded: bool,
    pub runs: Vec<TreeRun>,
}

/// A run leaf in the workspace tree.
#[derive(Clone)]
pub struct TreeRun {
    pub id: i64,
    pub agent: String,
    pub status: RunStatus,
}

/// One result in the project-wide search surface.
#[derive(Clone)]
pub enum SearchResult {
    File(search::Match),
    Note(notes::Hit),
    Record(store::SearchRecord),
}

/// An account with its latest usage snapshot, for the Accounts view.
#[derive(Clone)]
pub struct AccountRow {
    pub account: Account,
    pub usage: Option<Usage>,
}

/// A run as the fleet view needs it.
#[derive(Clone)]
pub struct RunRow {
    pub id: i64,
    pub agent: String,
    pub branch: String,
    pub worktree: String,
    pub status: RunStatus,
    /// Live semantic activity token (`working`/`blocked`/`done`) while running.
    pub activity: Option<String>,
    pub selected: bool,
    pub attempt: u32,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub exit_code: Option<i32>,
    pub output: String,
    pub error: Option<String>,
    pub files: usize,
    pub added: usize,
    pub removed: usize,
    pub checks: usize,
    pub check_status: Option<checks::Status>,
    pub checking: bool,
    pub terminal: Option<gpui::Entity<TermView>>,
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
            run_saved_at: std::collections::HashMap::new(),
            run_save_failed: std::collections::HashSet::new(),
            launch_needed: true,
            fanout_in_progress: false,
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

    /// Inspect a selected folder without mutating it. Existing repositories
    /// open immediately; plain folders wait for explicit initialization consent.
    pub fn consider_project_path(
        &mut self,
        path: std::path::PathBuf,
    ) -> Result<Option<i64>, String> {
        if git::is_repo(&path) {
            self.add_project_from_path(path, false).map(Some)
        } else {
            self.pending_project = Some(path);
            Ok(None)
        }
    }

    pub fn initialize_pending_project(&mut self) -> Result<i64, String> {
        let path = self
            .pending_project
            .take()
            .ok_or("no folder is waiting for initialization")?;
        self.add_project_from_path(path, true)
    }

    /// Add an inspected project. Initializing a plain folder is only allowed
    /// when the onboarding confirmation passed `initialize = true`.
    pub fn add_project_from_path(
        &mut self,
        path: std::path::PathBuf,
        initialize: bool,
    ) -> Result<i64, String> {
        let initialized = if !git::is_repo(&path) {
            if !initialize {
                return Err("this folder is not a git repository".into());
            }
            git::init_repo(&path).map_err(|e| e.to_string())?;
            true
        } else {
            false
        };
        let root = git::toplevel(&path).unwrap_or(path);
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".to_string());
        let base = git::current_branch(&root).unwrap_or_else(|| "main".to_string());
        let path_str = root.to_string_lossy().into_owned();
        let now = now();
        let project = self
            .db
            .create_project(&name, &path_str, &base, now)
            .map_err(|e| e.to_string())?;
        self.db
            .touch_project(project.id, now)
            .map_err(|error| error.to_string())?;
        self.select_project(project.id);
        self.pending_project = None;
        self.open_kind(TabKind::Tasks);
        if initialized {
            self.push_notification(
                "run_started",
                "Initialized a git repository",
                &format!("{name} is now tracked by git"),
                None,
            );
        }
        Ok(project.id)
    }

    /// True when there are no projects yet (show the onboarding empty state).
    pub fn is_empty(&self) -> bool {
        self.db.projects().map(|p| p.is_empty()).unwrap_or(true)
    }

    /// Create a task in the selected project from a prompt, select it, and (when
    /// `fan_out`) immediately fan it out across the chosen agents.
    pub fn create_task(
        &mut self,
        prompt: &str,
        fan_out: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            self.push_error(
                "Task is empty",
                "Describe the outcome before saving or running the task.",
            );
            return;
        }
        let Some(pid) = self.project_id else {
            self.push_error(
                "No project selected",
                "Open a repository before creating a task.",
            );
            return;
        };
        // The title is the first line, trimmed to something readable.
        let title: String = prompt
            .lines()
            .next()
            .unwrap_or(prompt)
            .chars()
            .take(60)
            .collect();
        match self.db.create_task(pid, &title, prompt, now()) {
            Ok(task) => {
                self.task_id = Some(task.id);
                self.selected_run_id = None;
                self.expanded.insert(format!("project-{pid}"));
                if fan_out {
                    self.run_fanout(window, cx);
                }
            }
            Err(error) => self.push_error("Could not create task", error.to_string()),
        }
    }

    /// Remove a project and everything under it.
    pub fn remove_project(&mut self, id: i64) {
        let Ok(project) = self.db.project(id) else {
            self.push_error("Could not remove project", "The project no longer exists.");
            return;
        };
        let runs: Vec<store::Run> = self
            .db
            .tasks(id)
            .unwrap_or_default()
            .into_iter()
            .flat_map(|task| self.db.runs(task.id).unwrap_or_default())
            .collect();
        if let Some(run) = runs
            .iter()
            .find(|run| matches!(run.status, RunStatus::Queued | RunStatus::Running))
        {
            self.push_error(
                "Project has active runs",
                format!("Cancel run {} before removing the project.", run.id),
            );
            return;
        }
        if let Some(run) = runs.iter().find(|run| {
            std::path::Path::new(&run.worktree).exists()
                && git::status::status(std::path::Path::new(&run.worktree))
                    .map(|entries| !entries.is_empty())
                    .unwrap_or(true)
        }) {
            self.push_error(
                "Project has a dirty worktree",
                format!(
                    "Review or explicitly remove {} before deleting its history.",
                    run.worktree
                ),
            );
            return;
        }
        for run in &runs {
            self.run_terms.remove(&run.id);
            if std::path::Path::new(&run.worktree).exists() {
                if let Err(error) = git::worktree::remove(
                    std::path::Path::new(&project.path),
                    std::path::Path::new(&run.worktree),
                    false,
                ) {
                    self.push_error("Could not clean project worktree", error.to_string());
                    return;
                }
            }
        }
        if let Err(error) = self.db.delete_project(id) {
            self.push_error("Could not remove project", error.to_string());
            return;
        }
        if self.project_id == Some(id) {
            self.project_id = self
                .db
                .projects()
                .ok()
                .and_then(|p| p.first().map(|p| p.id));
            self.task_id = self
                .project_id
                .and_then(|pid| self.db.tasks(pid).ok())
                .and_then(|t| t.first().map(|t| t.id));
            self.selected_run_id = self
                .task_id
                .and_then(|task_id| self.db.runs(task_id).ok())
                .and_then(|runs| runs.first().map(|run| run.id));
        }
    }

    /// Delete a task.
    pub fn delete_task(&mut self, id: i64) {
        let Ok(task) = self.db.task(id) else {
            self.push_error("Could not delete task", "The task no longer exists.");
            return;
        };
        let Ok(project) = self.db.project(task.project_id) else {
            self.push_error(
                "Could not delete task",
                "The task's project no longer exists.",
            );
            return;
        };
        let runs = self.db.runs(id).unwrap_or_default();
        if let Some(run) = runs
            .iter()
            .find(|run| matches!(run.status, RunStatus::Queued | RunStatus::Running))
        {
            self.push_error(
                "Task has an active run",
                format!("Cancel run {} before deleting the task.", run.id),
            );
            return;
        }
        if let Some(run) = runs.iter().find(|run| {
            std::path::Path::new(&run.worktree).exists()
                && git::status::status(std::path::Path::new(&run.worktree))
                    .map(|entries| !entries.is_empty())
                    .unwrap_or(true)
        }) {
            self.push_error(
                "Task has a dirty worktree",
                format!(
                    "Review or explicitly remove {} before deleting its history.",
                    run.worktree
                ),
            );
            return;
        }
        for run in runs {
            self.run_terms.remove(&run.id);
            if std::path::Path::new(&run.worktree).exists() {
                if let Err(error) = git::worktree::remove(
                    std::path::Path::new(&project.path),
                    std::path::Path::new(&run.worktree),
                    false,
                ) {
                    self.push_error("Could not clean task worktree", error.to_string());
                    return;
                }
            }
        }
        if let Err(error) = self.db.delete_task(id) {
            self.push_error("Could not delete task", error.to_string());
            return;
        }
        if self.task_id == Some(id) {
            self.task_id = self
                .project_id
                .and_then(|pid| self.db.tasks(pid).ok())
                .and_then(|t| t.first().map(|t| t.id));
            self.selected_run_id = self
                .task_id
                .and_then(|task_id| self.db.runs(task_id).ok())
                .and_then(|runs| runs.first().map(|run| run.id));
        }
    }

    /// Archive a task (set aside without deleting).
    pub fn archive_task(&mut self, id: i64) {
        if self
            .db
            .runs(id)
            .unwrap_or_default()
            .iter()
            .any(|run| matches!(run.status, RunStatus::Queued | RunStatus::Running))
        {
            self.push_error(
                "Task is active",
                "Cancel its queued and running agents before archiving it.",
            );
            return;
        }
        if let Err(error) = self.db.set_task_status(id, TaskStatus::Archived, now()) {
            self.push_error("Could not archive task", error.to_string());
        }
    }

    /// The filesystem path of a project.
    pub fn project_path_of(&self, id: i64) -> Option<String> {
        self.db.project(id).ok().map(|p| p.path)
    }

    /// Toggle a tree node's expanded state.
    pub fn toggle_expanded(&mut self, id: &str) {
        if !self.expanded.remove(id) {
            self.expanded.insert(id.to_string());
        }
    }

    /// Build the workspace tree (projects → tasks → runs), honoring which nodes
    /// are expanded. The selected project is expanded by default the first time.
    pub fn tree(&self) -> Vec<TreeProject> {
        let selected_project = format!("project-{}", self.project_id.unwrap_or(-1));
        self.db
            .projects()
            .unwrap_or_default()
            .into_iter()
            .map(|p| {
                let pkey = format!("project-{}", p.id);
                let expanded = self.expanded.contains(&pkey) || pkey == selected_project;
                let tasks = if expanded {
                    self.db
                        .tasks(p.id)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|t| {
                            let tkey = format!("task-{}", t.id);
                            let texpanded = self.expanded.contains(&tkey);
                            let runs = if texpanded {
                                self.db
                                    .runs(t.id)
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|r| TreeRun {
                                        id: r.id,
                                        agent: r.agent,
                                        status: r.status,
                                    })
                                    .collect()
                            } else {
                                Vec::new()
                            };
                            TreeTask {
                                id: t.id,
                                title: t.title,
                                status: t.status,
                                expanded: texpanded,
                                runs,
                            }
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                TreeProject {
                    id: p.id,
                    name: p.name,
                    pinned: p.pinned,
                    expanded,
                    tasks,
                }
            })
            .collect()
    }

    /// The id of the run currently under review (the selected task's first run).
    pub fn current_run_id(&self) -> Option<i64> {
        let task_id = self.task_id?;
        if let Some(id) = self.selected_run_id {
            if self
                .db
                .run(id)
                .ok()
                .is_some_and(|run| run.task_id == task_id)
            {
                return Some(id);
            }
        }
        self.db
            .runs(task_id)
            .ok()
            .and_then(|runs| runs.first().map(|run| run.id))
    }

    /// Annotations on the run under review.
    pub fn review_annotations(&self) -> Vec<store::Annotation> {
        self.current_run_id()
            .and_then(|rid| self.db.annotations(rid).ok())
            .unwrap_or_default()
    }

    /// Anchor the next review comment to a diff line (click a line to target).
    pub fn target_review_line(&mut self, file: &str, line: u32, side: store::Side) {
        self.review_target = Some((file.to_string(), line, side));
    }

    /// Add a review comment on the targeted diff line - a real annotation the
    /// store persists and the "send to agent" flow collects. With no target it
    /// falls back to the first changed file, line 1.
    pub fn add_review_note(&mut self, body: &str) {
        if body.trim().is_empty() {
            return;
        }
        let Some(rid) = self.current_run_id() else {
            return;
        };
        let (file, line, side) = self.review_target.take().unwrap_or_else(|| {
            let file = self
                .review_diff()
                .first()
                .map(|f| f.path.clone())
                .unwrap_or_else(|| "(file)".to_string());
            (file, 1, store::Side::New)
        });
        if let Err(error) = self.db.add_annotation(rid, &file, line, side, body, now()) {
            self.push_error("Could not add review comment", error.to_string());
        }
    }

    /// Mark a review annotation resolved or reopen it.
    pub fn resolve_review_note(&mut self, id: i64, resolved: bool) {
        if let Err(error) = self.db.resolve_annotation(id, resolved) {
            self.push_error("Could not update review comment", error.to_string());
        }
    }

    /// Delete a review annotation.
    pub fn delete_review_note(&mut self, id: i64) {
        if let Err(error) = self.db.delete_annotation(id) {
            self.push_error("Could not delete review comment", error.to_string());
        }
    }

    /// Continue the selected run with its open review annotations. A live
    /// agent receives them on stdin; a finished run starts another attempt in
    /// the same worktree.
    pub fn send_review_to_agent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let annotations: Vec<store::Annotation> = self
            .review_annotations()
            .into_iter()
            .filter(|a| !a.resolved)
            .collect();
        if annotations.is_empty() {
            return;
        }
        let Some(run_id) = self.current_run_id() else {
            return;
        };
        let mut prompt = String::from("Address these review comments:\n");
        for a in &annotations {
            prompt.push_str(&format!("- {}:{} — {}\n", a.file, a.line, a.body));
        }
        match self.send_followup(run_id, prompt, window, cx) {
            Ok(()) => {
                for annotation in &annotations {
                    let _ = self.db.resolve_annotation(annotation.id, true);
                }
                self.open_kind(TabKind::Tasks);
                self.push_notice(
                    crate::run::NoticeTone::Info,
                    "Review sent",
                    "The selected agent is addressing the comments in the same worktree.",
                );
            }
            Err(error) => self.push_error("Could not send review", error),
        }
    }

    /// Create a worktree + task from a GitHub issue (open a worktree from
    /// a task"). Best effort on the worktree; the task is always created.
    pub fn create_worktree_from_issue(&mut self, issue: &github::Issue) {
        let Some(pid) = self.project_id else {
            return;
        };
        let Ok(project) = self.db.project(pid) else {
            return;
        };
        let repo = std::path::PathBuf::from(&project.path);
        let branch = github::issue_branch(issue);
        let worktree = format!(".asylum/worktrees/{branch}");
        let absolute = match git::worktree::create(&repo, &worktree, Some(&branch), None) {
            Ok(path) => path,
            Err(error) => {
                self.push_error("Could not open issue worktree", error.to_string());
                return;
            }
        };
        let prompt = format!("Resolve GitHub issue #{}: {}", issue.number, issue.title);
        if let Ok(task) = self.db.create_task(pid, &issue.title, &prompt, now()) {
            self.task_id = Some(task.id);
            self.push_notice(
                crate::run::NoticeTone::Success,
                "Issue worktree ready",
                absolute.display().to_string(),
            );
            self.open_kind(TabKind::Tasks);
        }
    }

    /// Open a pull request for a run's branch (open a PR from the IDE).
    pub fn create_pr_for_run(&mut self, run_id: i64) {
        let Ok(run) = self.db.run(run_id) else {
            self.push_error("Run unavailable", "The selected run no longer exists.");
            return;
        };
        let Ok(task) = self.db.task(run.task_id) else {
            self.push_error("Task unavailable", "The run's task no longer exists.");
            return;
        };
        let Ok(project) = self.db.project(task.project_id) else {
            self.push_error(
                "Project unavailable",
                "The task's project no longer exists.",
            );
            return;
        };
        if run.status != RunStatus::Succeeded {
            self.push_error(
                "Run is not ready",
                "Only a successful run can be opened as a pull request.",
            );
            return;
        }
        if self.checking_runs.contains(&run_id) {
            self.push_error(
                "Checks are still running",
                "Wait for verification to finish before opening the pull request.",
            );
            return;
        }
        let results = self.run_check_results(run_id);
        if results
            .iter()
            .any(|result| result.status == checks::Status::Fail)
        {
            self.push_error(
                "Checks failed",
                "Fix or rerun failed checks before opening the pull request.",
            );
            return;
        }
        if results.is_empty() || checks::overall(&results) == checks::Status::Skipped {
            self.push_notice(
                crate::run::NoticeTone::Warning,
                "Run is not fully verified",
                "No executable checks passed. Review the diff and terminal output carefully.",
            );
        }
        let repo = std::path::PathBuf::from(&project.path);
        let base = config::load_project(&repo)
            .0
            .base_branch
            .unwrap_or(project.base_branch);
        match github::create_pr(&repo, &task.title, &task.prompt, &base, &run.branch) {
            Ok(url) => {
                self.reference_run_notes(run_id, &notes::Reference::pullrequest(&url));
                self.push_notice(
                    crate::run::NoticeTone::Success,
                    "Pull request opened",
                    url.clone(),
                );
                self.push_notification("run_finished", "PR opened", &url, Some(run_id));
            }
            Err(error) => {
                self.push_error("Could not open pull request", error.to_string());
                self.push_notification("attention", "PR failed", &error.to_string(), Some(run_id));
            }
        }
    }

    /// The project's local branches (for the branch list in the review view).
    pub fn branches(&self) -> Vec<git::Branch> {
        let dir = std::path::PathBuf::from(self.project_path());
        git::branch::branches(&dir).unwrap_or_default()
    }

    /// Open the file `name` in a new editor tab.
    pub fn open_file(&mut self, name: &str, cx: &mut Context<Self>) {
        self.editor_file = Some(name.to_string());
        self.open_editor(name, cx);
    }

    // ── Tab opening ─────────────────────────────────────────────────────────

    /// Open (or focus) the tab for a nav-menu [`View`].
    pub fn open_view(&mut self, v: View, window: &mut Window, cx: &mut Context<Self>) {
        match v {
            View::Tasks => self.open_kind(TabKind::Tasks),
            View::Diff => self.open_kind(TabKind::Diff),
            View::Search => self.open_kind(TabKind::Search),
            View::Notes => self.open_kind(TabKind::Notes),
            View::Integrations => self.open_kind(TabKind::Integrations),
            View::Accounts => self.open_kind(TabKind::Accounts),
            View::Notifications => self.open_kind(TabKind::Inbox),
            View::Plugins => self.open_kind(TabKind::Plugins),
            View::Terminal => self.open_terminal(window, cx),
            View::Editor => {
                if let Some(f) = self.project_files().first().cloned() {
                    self.open_editor(&f, cx);
                }
            }
            View::Preview => self.open_preview(cx),
            View::Browser => self.open_browser(cx),
            View::Settings => self.open_kind(TabKind::Settings),
        }
    }

    pub(crate) fn open_kind(&mut self, kind: TabKind) {
        let id = self.next_tab_id();
        self.workspace.open(id, kind);
    }

    /// Open a terminal tab running in the selected project.
    pub fn open_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self.project_path();
        let mut opts = libsinclair::terminal::SessionOptions::default();
        opts.spawn.cwd = Some(cwd.into());
        let term = cx.new(|cx| {
            TermView::spawn(opts, TermOptions::default(), window, cx).expect("spawn terminal")
        });
        let focus = term.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        let id = self.next_tab_id();
        self.workspace.open(id, TabKind::Terminal(term));
    }

    /// Open an editor tab for a project file.
    pub fn open_editor(&mut self, file: &str, cx: &mut Context<Self>) {
        let content = self.read_project_file(file);
        let is_rust = file.ends_with(".rs");
        let editor = cx.new(|cx| {
            let e = guise::Editor::new(cx).value(content.as_str());
            if is_rust {
                e.language(guise::Language::Rust)
            } else {
                e
            }
        });
        let id = self.next_tab_id();
        self.workspace
            .open(id, TabKind::Editor(editor, file.to_string()));
    }

    /// Open a browser tab with design mode on - click an element, attach a
    /// note, and collect numbered annotations for "send to agent".
    pub fn open_browser(&mut self, cx: &mut Context<Self>) {
        let wv = cx.new(|cx| {
            guise::WebView::new(cx)
                .init_script(designmode::INJECT_JS)
                .url("https://example.com")
        });
        self.design_enabled.insert(wv.entity_id());
        self.watch_design_messages(&wv, cx);
        let id = self.next_tab_id();
        self.workspace.open(id, TabKind::Browser(wv));
    }

    /// Open a preview tab (the open editor file, or the project README).
    /// Design mode is available from the toolbar but starts off.
    pub fn open_preview(&mut self, cx: &mut Context<Self>) {
        let html = self.preview_html();
        let wv = cx.new(|cx| {
            guise::WebView::new(cx)
                .init_script(designmode::INJECT_JS)
                .html(html)
        });
        self.watch_design_messages(&wv, cx);
        let id = self.next_tab_id();
        self.workspace.open(id, TabKind::Preview(wv));
    }

    /// Route a web view's design-mode traffic: a capture becomes the pending
    /// annotation, and every page load re-asserts the design-mode toggle and
    /// redraws the pins (navigation wipes the page's state, not ours).
    fn watch_design_messages(&mut self, wv: &gpui::Entity<guise::WebView>, cx: &mut Context<Self>) {
        cx.subscribe(
            wv,
            |root, wv, event: &guise::WebViewEvent, cx| match event {
                guise::WebViewEvent::Message(payload) => {
                    if let Some(capture) = designmode::parse(payload) {
                        root.pending_capture = Some(capture);
                        cx.notify();
                    }
                }
                guise::WebViewEvent::LoadFinished => {
                    if root.design_enabled.contains(&wv.entity_id()) {
                        wv.read(cx).evaluate_script(designmode::ENABLE_JS);
                    }
                    if !root.design_annotations.is_empty() {
                        wv.read(cx)
                            .evaluate_script(&designmode::pins_js(&root.design_annotations));
                    }
                }
                _ => {}
            },
        )
        .detach();
    }

    /// Attach a note to the pending capture, making it a numbered annotation.
    /// Returns the new annotation's (selector, number) so the view can pin it.
    pub fn attach_design_note(&mut self, note: &str) -> Option<(String, usize)> {
        let capture = self.pending_capture.take()?;
        let selector = capture.selector.clone();
        self.design_annotations.push(designmode::Annotation {
            capture,
            note: note.trim().to_string(),
        });
        Some((selector, self.design_annotations.len()))
    }

    /// Drop a design annotation (the view renumbers the pins via `pins_js`).
    pub fn remove_design_annotation(&mut self, index: usize) {
        if index < self.design_annotations.len() {
            self.design_annotations.remove(index);
        }
    }

    /// Ship the collected design annotations to an agent as a new task, then
    /// switch to the Tasks board.
    pub fn send_design_to_agent(&mut self) {
        if self.design_annotations.is_empty() {
            return;
        }
        let Some(pid) = self.project_id else {
            return;
        };
        let prompt = designmode::to_prompt_many(&self.design_annotations);
        let title = match self.design_annotations.as_slice() {
            [a] => format!("Design: {}", a.capture.selector),
            many => format!("Design: {} annotations", many.len()),
        };
        if let Ok(task) = self.db.create_task(pid, &title, &prompt, now()) {
            self.task_id = Some(task.id);
            self.design_annotations.clear();
            self.pending_capture = None;
            self.open_kind(TabKind::Tasks);
        }
    }

    /// Top-level files of the selected project worth opening in the editor
    /// (texty files and small config), sorted, capped.
    pub fn project_files(&self) -> Vec<String> {
        let dir = std::path::PathBuf::from(self.project_path());
        let mut files: Vec<String> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_file())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| !name.starts_with('.'))
            .collect();
        files.sort();
        files.truncate(200);
        files
    }

    /// Read a project file's contents for the editor (empty on error).
    pub fn read_project_file(&self, name: &str) -> String {
        let path = std::path::PathBuf::from(self.project_path()).join(name);
        std::fs::read_to_string(path).unwrap_or_default()
    }

    /// A full HTML preview document for the Preview surface: the file open in
    /// the editor (markdown / image / PDF / text), else the project README.
    pub fn preview_html(&self) -> String {
        let dir = std::path::PathBuf::from(self.project_path());
        if let Some(name) = &self.editor_file {
            if let Ok(html) = preview::html_document(&dir.join(name)) {
                return html;
            }
        }
        for candidate in ["README.md", "readme.md", "Readme.md"] {
            let path = dir.join(candidate);
            if path.exists() {
                if let Ok(html) = preview::html_document(&path) {
                    return html;
                }
            }
        }
        preview::html_document(std::path::Path::new("/nonexistent")).unwrap_or_else(|_| {
            "<!doctype html><p>Nothing to preview. Open a file in the editor.</p>".to_string()
        })
    }

    /// The filesystem path of the selected project (or the cwd as a fallback).
    pub fn project_path(&self) -> String {
        self.project_id
            .and_then(|id| self.db.project(id).ok())
            .map(|p| p.path)
            .unwrap_or_else(|| ".".to_string())
    }

    /// Load GitHub PRs and issues for the selected project via the `gh` CLI.
    /// Errors (no `gh`, no auth, not a GitHub repo) are captured for display
    /// rather than surfaced as a crash.
    pub fn load_github(&mut self) {
        let Some(pid) = self.project_id else {
            return;
        };
        let Ok(project) = self.db.project(pid) else {
            return;
        };
        let dir = std::path::PathBuf::from(&project.path);
        self.integration_error = None;
        match github::pull_requests(&dir, 30) {
            Ok(prs) => self.prs = prs,
            Err(e) => self.integration_error = Some(e.to_string()),
        }
        match github::issues(&dir, 30) {
            Ok(issues) => self.issues = issues,
            Err(e) => {
                if self.integration_error.is_none() {
                    self.integration_error = Some(e.to_string());
                }
            }
        }
        self.load_linear();
    }

    /// Load Linear issues when an API token is configured. Absent token leaves
    /// the list empty (the surface then shows how to set one); an API error is
    /// captured for display, never a crash.
    pub fn load_linear(&mut self) {
        let token = self.settings.linear_token.trim();
        if token.is_empty() {
            self.linear_issues.clear();
            return;
        }
        match linear::Client::new(token).issues() {
            Ok(issues) => self.linear_issues = issues,
            Err(e) => {
                if self.integration_error.is_none() {
                    self.integration_error = Some(format!("Linear: {e}"));
                }
            }
        }
    }

    /// Create a worktree + task from a Linear issue, mirroring the GitHub flow.
    pub fn create_worktree_from_linear_issue(&mut self, issue: &linear::Issue) {
        let Some(pid) = self.project_id else {
            return;
        };
        let Ok(project) = self.db.project(pid) else {
            return;
        };
        let repo = std::path::PathBuf::from(&project.path);
        let branch = linear_branch(issue);
        let worktree = format!(".asylum/worktrees/{branch}");
        let absolute = match git::worktree::create(&repo, &worktree, Some(&branch), None) {
            Ok(path) => path,
            Err(error) => {
                self.push_error("Could not open issue worktree", error.to_string());
                return;
            }
        };
        let prompt = format!("Resolve Linear issue {}: {}", issue.identifier, issue.title);
        if let Ok(task) = self.db.create_task(pid, &issue.title, &prompt, now()) {
            self.task_id = Some(task.id);
            self.push_notice(
                crate::run::NoticeTone::Success,
                "Issue worktree ready",
                absolute.display().to_string(),
            );
            self.open_kind(TabKind::Tasks);
        }
    }

    /// Add a provider account from the `provider: label` input. The first run
    /// against an agent verifies the credential; usage fills in when a provider
    /// reports it. The newly added account becomes active.
    pub fn add_account_from_input(&mut self, cx: &mut gpui::Context<Self>) {
        let Some(input) = self.account_input.clone() else {
            return;
        };
        let raw = input.read(cx).text();
        let (provider, label) = match raw.split_once(':') {
            Some((provider, label)) => (provider.trim(), label.trim()),
            None => (raw.trim(), "default"),
        };
        if provider.is_empty() {
            self.push_error("Account needs a provider", "Write it as provider: label.");
            return;
        }
        let label = if label.is_empty() { "default" } else { label };
        match self.db.add_account(provider, label, now()) {
            Ok(account) => {
                let _ = self.db.activate_account(account.id);
                input.update(cx, |input, cx| input.set_text("", cx));
                cx.notify();
            }
            Err(error) => self.push_error("Could not add account", error.to_string()),
        }
    }

    /// Accounts with their latest usage, grouped for the Accounts view.
    pub fn accounts(&self) -> Vec<AccountRow> {
        self.db
            .accounts(None)
            .unwrap_or_default()
            .into_iter()
            .map(|account| AccountRow {
                usage: self.db.latest_usage(account.id).ok().flatten(),
                account,
            })
            .collect()
    }

    /// The notification inbox, newest first.
    pub fn notifications(&self) -> Vec<Notification> {
        self.db.notifications(false).unwrap_or_default()
    }

    /// Unread notification count (badge).
    pub fn unread(&self) -> usize {
        self.db.unread_count().unwrap_or(0)
    }

    /// The diff under review for the selected run - the run's worktree diffed
    /// against where it forked from the project's base branch.
    pub fn review_diff(&self) -> Vec<git::DiffFile> {
        let Some(rid) = self.current_run_id() else {
            return Vec::new();
        };
        self.diff_for_run(rid)
    }

    /// A run's worktree diffed from the configured project base.
    pub fn diff_for_run(&self, rid: i64) -> Vec<git::DiffFile> {
        let Ok(run) = self.db.run(rid) else {
            return Vec::new();
        };
        let wt = std::path::PathBuf::from(&run.worktree);
        if !git::is_repo(&wt) {
            return Vec::new();
        }
        let project = self
            .db
            .task(run.task_id)
            .ok()
            .and_then(|t| self.db.project(t.project_id).ok());
        let base = project
            .map(|project| {
                config::load_project(std::path::Path::new(&project.path))
                    .0
                    .base_branch
                    .unwrap_or(project.base_branch)
            })
            .unwrap_or_else(|| "HEAD".to_string());
        git::diff::since_fork(&wt, &base)
            .or_else(|_| git::diff::against(&wt, "HEAD"))
            .unwrap_or_default()
    }

    /// Snapshot the runs of the selected task.
    pub fn runs(&self) -> Vec<RunRow> {
        let Some(tid) = self.task_id else {
            return Vec::new();
        };
        self.db
            .runs(tid)
            .unwrap_or_default()
            .into_iter()
            .map(|run| {
                let files = self.diff_for_run(run.id);
                let check_results = self.run_check_results(run.id);
                let (added, removed) = files
                    .iter()
                    .map(git::DiffFile::line_stats)
                    .fold((0, 0), |(a, r), (next_a, next_r)| (a + next_a, r + next_r));
                RunRow {
                    id: run.id,
                    agent: run.agent,
                    branch: run.branch,
                    worktree: run.worktree,
                    status: run.status,
                    activity: run.activity,
                    selected: self.current_run_id() == Some(run.id),
                    attempt: run.attempt,
                    started_at: run.started_at,
                    ended_at: run.ended_at,
                    exit_code: run.exit_code,
                    output: run.output,
                    error: run.error,
                    files: files.len(),
                    added,
                    removed,
                    checks: check_results.len(),
                    check_status: (!check_results.is_empty())
                        .then(|| checks::overall(&check_results)),
                    checking: self.checking_runs.contains(&run.id),
                    terminal: self.run_terms.get(&run.id).cloned(),
                }
            })
            .collect()
    }

    /// Search source files, notes, tasks, runs, and persisted transcripts.
    pub fn run_search(&mut self) {
        let Some(pid) = self.project_id else {
            return;
        };
        let Ok(project) = self.db.project(pid) else {
            return;
        };
        let query = self.search_query.trim().to_string();
        self.search_results.clear();
        if let Ok(records) = self.db.search_project(pid, &query, 120) {
            self.search_results
                .extend(records.into_iter().map(SearchResult::Record));
        }
        let root = self
            .db
            .note_vault(pid)
            .ok()
            .flatten()
            .map(|vault| std::path::PathBuf::from(vault.path));
        if let Some(root) = root {
            if let Ok(index) = notes::index(&root) {
                self.search_results.extend(
                    notes::search(&index, &query)
                        .into_iter()
                        .take(120)
                        .map(SearchResult::Note),
                );
            }
        }
        if query.is_empty() {
            return;
        }
        let dir = std::path::PathBuf::from(&project.path);
        let options = search::Options {
            fixed: true,
            max_results: 200,
            ..Default::default()
        };
        match search::search(&dir, &query, &options) {
            Ok(results) => self
                .search_results
                .extend(results.into_iter().map(SearchResult::File)),
            Err(error) => self.push_error("Search failed", error.to_string()),
        }
    }

    /// Store a notification and post a desktop notification for it.
    pub(crate) fn push_notification(
        &self,
        kind: &str,
        title: &str,
        body: &str,
        run_id: Option<i64>,
    ) {
        let _ = self.db.notify(kind, title, body, run_id, now());
        let _ = notify::send(&notify::Notification::new(title, body));
    }

    /// Installed plugins (from the plugins directory) and any load diagnostics.
    pub fn plugins(&self) -> plugin::Installed {
        plugin::load_dir(&plugin::default_dir())
    }

    /// The plugins directory path, for display in the Plugins view.
    pub fn plugins_dir(&self) -> String {
        plugin::default_dir().to_string_lossy().into_owned()
    }

    /// Invoke a plugin command through its declared runtime (process or WASM),
    /// passing the current project/task as context. Runs synchronously; the
    /// result (or error) surfaces as a notice.
    pub fn run_plugin_command(
        &mut self,
        plugin_id: &str,
        method: &str,
        cx: &mut gpui::Context<Self>,
    ) {
        let installed = self.plugins();
        let Some(plugin) = installed.plugins.iter().find(|p| p.id == plugin_id) else {
            self.push_error(
                "Plugin unavailable",
                format!("{plugin_id} is no longer installed."),
            );
            return;
        };
        let Some(runtime) = &plugin.runtime else {
            self.push_error(
                "Plugin has no runtime",
                format!(
                    "{} declares commands but no [runtime] to run them.",
                    plugin.name
                ),
            );
            return;
        };
        let project = self.project_id.and_then(|id| self.db.project(id).ok());
        let params = serde_json::json!({
            "command": method,
            "project": project.as_ref().map(|p| p.path.clone()),
            "task": self.task_id,
        });
        let cwd = project
            .as_ref()
            .map(|p| std::path::PathBuf::from(&p.path))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let result = match runtime.kind {
            plugin::RuntimeKind::Process => pluginrt::invoke_once(runtime, &cwd, method, params),
            plugin::RuntimeKind::Wasm => {
                pluginrt::invoke_wasm(runtime, &plugin.path, &plugin.capabilities, method, &params)
            }
        };
        match result {
            Ok(value) => {
                let summary: String = value.to_string().chars().take(200).collect();
                self.push_notice(
                    crate::run::NoticeTone::Success,
                    format!("{} ran", plugin.name),
                    summary,
                );
            }
            Err(error) => self.push_error("Plugin command failed", error.to_string()),
        }
        cx.notify();
    }

    /// Create a task from dropped file paths (drag-drop into the prompt).
    pub fn create_task_from_files(&mut self, paths: &[std::path::PathBuf]) {
        let Some(pid) = self.project_id else {
            return;
        };
        if paths.is_empty() {
            return;
        }
        let names: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        let title = format!(
            "Work on {}",
            paths
                .first()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "dropped files".into())
        );
        let prompt = format!("Consider these files:\n{}", names.join("\n"));
        if let Ok(task) = self.db.create_task(pid, &title, &prompt, now()) {
            self.task_id = Some(task.id);
            self.open_kind(TabKind::Tasks);
        }
    }

    /// Toggle a project's pinned state.
    pub fn toggle_pin(&mut self, project_id: i64) {
        if let Ok(project) = self.db.project(project_id) {
            let _ = self.db.set_pinned(project_id, !project.pinned);
        }
    }

    /// Select a project, stamp it as recently opened, and default the task
    /// selection to its first task.
    pub fn select_project(&mut self, project_id: i64) {
        self.project_id = Some(project_id);
        let _ = self.db.touch_project(project_id, now());
        self.task_id = self
            .db
            .tasks(project_id)
            .ok()
            .and_then(|t| t.first().map(|t| t.id));
        self.selected_run_id = self
            .task_id
            .and_then(|tid| self.db.runs(tid).ok())
            .and_then(|runs| runs.first().map(|run| run.id));
        let path = self.db.project(project_id).ok().map(|project| project.path);
        if let Some(path) = path {
            let (project, diagnostics) = config::load_project(std::path::Path::new(&path));
            if !project.default_agents.is_empty() {
                self.fanout = project.default_agents;
            }
            for diagnostic in diagnostics {
                self.push_error("Project settings", diagnostic.message);
            }
        }
        // Quick-open lists this project's files; rebuild it.
        self.editor_file = None;
        self.quickopen = None;
        self.note.project_id = None;
        self.refresh_setup();
    }

    /// The title of the selected task, if any.
    pub fn task_title(&self) -> Option<String> {
        self.task_id
            .and_then(|id| self.db.task(id).ok())
            .map(|t| t.title)
    }

    pub fn task_status(&self) -> Option<TaskStatus> {
        self.task_id
            .and_then(|id| self.db.task(id).ok())
            .map(|task| task.status)
    }
}
