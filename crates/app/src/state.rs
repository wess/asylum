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

/// The root application state.
pub struct Root {
    pub db: Db,
    pub project_id: Option<i64>,
    pub task_id: Option<i64>,
    /// Agent ids selected for the next fan-out.
    pub fanout: Vec<String>,
    /// Live search query and its results (Search view).
    pub search_query: String,
    pub search_results: Vec<search::Match>,
    /// GitHub PRs/issues for the Integrations view (loaded on demand).
    pub prs: Vec<github::PullRequest>,
    pub issues: Vec<github::Issue>,
    /// Last integration load error, if any.
    pub integration_error: Option<String>,
    /// The file last opened in an editor tab (tracked so Preview can follow it).
    pub editor_file: Option<String>,
    /// The latest check results (type-check / lint / test) for the review view.
    pub check_results: Vec<checks::CheckResult>,
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
    pub status: RunStatus,
}

impl Root {
    /// The on-disk store path: `$XDG_DATA_HOME/asylum/workspace.sqlite` (or
    /// `~/.local/share/asylum/...`). Shared with the companion server.
    pub fn db_path() -> std::path::PathBuf {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")))
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
        let db = Db::open(&path).or_else(|_| Db::memory()).expect("open store");
        let project_id = db.projects().ok().and_then(|p| p.first().map(|p| p.id));
        let task_id = project_id
            .and_then(|pid| db.tasks(pid).ok())
            .and_then(|t| t.first().map(|t| t.id));
        Root {
            db,
            project_id,
            task_id,
            fanout: vec!["claude-code".into(), "codex".into()],
            search_query: String::new(),
            search_results: Vec::new(),
            prs: Vec::new(),
            issues: Vec::new(),
            integration_error: None,
            editor_file: None,
            check_results: Vec::new(),
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
            workspace: crate::workspace::Workspace::new(0),
            next_tab_id: 1,
            settings: config::Settings::default(),
            settings_diagnostics: Vec::new(),
            settings_watch: None,
            settings_inputs: None,
        }
    }

    /// A fresh, monotonic tab id.
    pub fn next_tab_id(&mut self) -> u64 {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        id
    }

    /// Add a project from any folder. A git repository is used as-is; a plain
    /// folder is initialized as one (with an empty initial commit) so the ADE's
    /// worktree flows work. Selects the new project.
    pub fn add_project_from_path(&mut self, path: std::path::PathBuf) -> Result<i64, String> {
        let initialized = if !git::is_repo(&path) {
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
        self.db.touch_project(project.id, now).ok();
        self.select_project(project.id);
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
    pub fn create_task(&mut self, prompt: &str, fan_out: bool) {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return;
        }
        let Some(pid) = self.project_id else {
            return;
        };
        // The title is the first line, trimmed to something readable.
        let title: String = prompt.lines().next().unwrap_or(prompt).chars().take(60).collect();
        if let Ok(task) = self.db.create_task(pid, &title, prompt, now()) {
            self.task_id = Some(task.id);
            self.expanded.insert(format!("project-{pid}"));
            if fan_out {
                self.run_fanout();
            }
        }
    }

    /// Remove a project and everything under it.
    pub fn remove_project(&mut self, id: i64) {
        let _ = self.db.delete_project(id);
        if self.project_id == Some(id) {
            self.project_id = self.db.projects().ok().and_then(|p| p.first().map(|p| p.id));
            self.task_id = self
                .project_id
                .and_then(|pid| self.db.tasks(pid).ok())
                .and_then(|t| t.first().map(|t| t.id));
        }
    }

    /// Delete a task.
    pub fn delete_task(&mut self, id: i64) {
        let _ = self.db.delete_task(id);
        if self.task_id == Some(id) {
            self.task_id = self
                .project_id
                .and_then(|pid| self.db.tasks(pid).ok())
                .and_then(|t| t.first().map(|t| t.id));
        }
    }

    /// Archive a task (set aside without deleting).
    pub fn archive_task(&mut self, id: i64) {
        let _ = self.db.set_task_status(id, TaskStatus::Archived, now());
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
        self.task_id
            .and_then(|tid| self.db.runs(tid).ok())
            .and_then(|runs| runs.first().map(|r| r.id))
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
        let _ = self.db.add_annotation(rid, &file, line, side, body, now());
    }

    /// Mark a review annotation resolved or reopen it.
    pub fn resolve_review_note(&mut self, id: i64, resolved: bool) {
        let _ = self.db.resolve_annotation(id, resolved);
    }

    /// Delete a review annotation.
    pub fn delete_review_note(&mut self, id: i64) {
        let _ = self.db.delete_annotation(id);
    }

    /// Ship the open (unresolved) review annotations back to an agent as a
    /// follow-up task, then mark them resolved.
    pub fn send_review_to_agent(&mut self) {
        let annotations: Vec<store::Annotation> = self
            .review_annotations()
            .into_iter()
            .filter(|a| !a.resolved)
            .collect();
        if annotations.is_empty() {
            return;
        }
        let Some(pid) = self.project_id else {
            return;
        };
        let mut prompt = String::from("Address these review comments:\n");
        for a in &annotations {
            prompt.push_str(&format!("- {}:{} — {}\n", a.file, a.line, a.body));
        }
        if let Ok(task) = self.db.create_task(pid, "Address review comments", &prompt, now()) {
            for a in &annotations {
                let _ = self.db.resolve_annotation(a.id, true);
            }
            self.task_id = Some(task.id);
            self.open_kind(TabKind::Tasks);
            self.push_notification("run_started", "Review sent to agent", "", None);
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
        let _ = git::worktree::create(&repo, &worktree, Some(&branch), None);
        let prompt = format!("Resolve GitHub issue #{}: {}", issue.number, issue.title);
        if let Ok(task) = self.db.create_task(pid, &issue.title, &prompt, now()) {
            self.task_id = Some(task.id);
            self.open_kind(TabKind::Tasks);
        }
    }

    /// Open a pull request for a run's branch (open a PR from the IDE).
    pub fn create_pr_for_run(&mut self, run_id: i64) {
        let Ok(run) = self.db.run(run_id) else {
            return;
        };
        let Ok(task) = self.db.task(run.task_id) else {
            return;
        };
        let Ok(project) = self.db.project(task.project_id) else {
            return;
        };
        let repo = std::path::PathBuf::from(&project.path);
        match github::create_pr(&repo, &task.title, &task.prompt, &project.base_branch, &run.branch) {
            Ok(url) => self.push_notification("run_finished", "PR opened", &url, Some(run_id)),
            Err(e) => self.push_notification("attention", "PR failed", &e.to_string(), Some(run_id)),
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

    fn open_kind(&mut self, kind: TabKind) {
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
        self.workspace.open(id, TabKind::Editor(editor, file.to_string()));
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
        cx.subscribe(wv, |root, wv, event: &guise::WebViewEvent, cx| match event {
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
        })
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

    /// Detect and run the selected project's checks (type-check / lint / test),
    /// storing the PASS/FAIL results for the review surface.
    pub fn run_checks(&mut self) {
        let dir = std::path::PathBuf::from(self.project_path());
        let detected = checks::detect(&dir);
        self.check_results = checks::run_all(&dir, &detected);
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
        let Ok(run) = self.db.run(rid) else {
            return Vec::new();
        };
        let wt = std::path::PathBuf::from(&run.worktree);
        if !git::is_repo(&wt) {
            return Vec::new();
        }
        let base = self
            .db
            .task(run.task_id)
            .ok()
            .and_then(|t| self.db.project(t.project_id).ok())
            .map(|p| p.base_branch)
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
            .map(|r| RunRow {
                id: r.id,
                agent: r.agent,
                branch: r.branch,
                status: r.status,
            })
            .collect()
    }

    /// Run the current search query against the selected project's directory.
    /// An empty query defaults to `TODO` so the surface always demonstrates.
    pub fn run_search(&mut self) {
        let Some(pid) = self.project_id else {
            return;
        };
        let Ok(project) = self.db.project(pid) else {
            return;
        };
        let query = if self.search_query.trim().is_empty() {
            "TODO".to_string()
        } else {
            self.search_query.clone()
        };
        self.search_query = query.clone();
        let dir = std::path::PathBuf::from(&project.path);
        self.search_results =
            search::search(&dir, &query, &search::Options::default()).unwrap_or_default();
    }

    /// Fan the selected task out across the chosen agents: plan a branch +
    /// worktree per agent, create the worktree (best effort - a non-repo demo
    /// project just skips it), record a run row, move the task to Running, and
    /// post a notification - the core loop: one prompt → N isolated agents.
    pub fn run_fanout(&mut self) {
        let Some(tid) = self.task_id else {
            return;
        };
        let Ok(task) = self.db.task(tid) else {
            return;
        };
        let Ok(project) = self.db.project(task.project_id) else {
            return;
        };
        let repo = std::path::PathBuf::from(&project.path);
        let plans = agent::plan::fanout(tid, &task.title, &self.fanout, ".asylum/worktrees");
        for plan in &plans {
            let _ = git::worktree::create(&repo, &plan.worktree, Some(&plan.branch), None);
            let _ = self.db.create_run(tid, &plan.agent, &plan.worktree, &plan.branch);
        }
        let now = now();
        self.db.set_task_status(tid, TaskStatus::Running, now).ok();
        self.push_notification(
            "run_started",
            &format!("Fanned out to {} agents", plans.len()),
            &task.title,
            None,
        );
    }

    /// Store a notification and post a desktop notification for it.
    fn push_notification(&self, kind: &str, title: &str, body: &str, run_id: Option<i64>) {
        let _ = self.db.notify(kind, title, body, run_id, now());
        let _ = notify::send(&notify::Notification::new(title, body));
    }

    /// Merge a run's branch into its project's base branch - "merge the winner".
    /// Reports the outcome (merged / conflicts / error) as a notification.
    pub fn merge_run(&mut self, run_id: i64) {
        let Ok(run) = self.db.run(run_id) else {
            return;
        };
        let Ok(task) = self.db.task(run.task_id) else {
            return;
        };
        let Ok(project) = self.db.project(task.project_id) else {
            return;
        };
        let repo = std::path::PathBuf::from(&project.path);
        let now = now();
        let _ = git::branch::checkout(&repo, &project.base_branch);
        match git::branch::merge(&repo, &run.branch) {
            Ok(git::MergeOutcome::Conflicts(paths)) => {
                self.push_notification(
                    "attention",
                    "Merge conflicts",
                    &format!("{} · {} files conflict", run.agent, paths.len()),
                    Some(run_id),
                );
            }
            Ok(_) => {
                self.db.set_task_status(task.id, TaskStatus::Merged, now).ok();
                self.push_notification(
                    "run_finished",
                    "Merged",
                    &format!("{} merged into {}", run.agent, project.base_branch),
                    Some(run_id),
                );
            }
            Err(e) => {
                self.push_notification("attention", "Merge failed", &e.to_string(), Some(run_id));
            }
        }
    }

    /// Installed plugins (from the plugins directory) and any load diagnostics.
    pub fn plugins(&self) -> plugin::Installed {
        plugin::load_dir(&plugin::default_dir())
    }

    /// The plugins directory path, for display in the Plugins view.
    pub fn plugins_dir(&self) -> String {
        plugin::default_dir().to_string_lossy().into_owned()
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
        // Quick-open lists this project's files; rebuild it.
        self.editor_file = None;
        self.quickopen = None;
    }

    /// The title of the selected task, if any.
    pub fn task_title(&self) -> Option<String> {
        self.task_id
            .and_then(|id| self.db.task(id).ok())
            .map(|t| t.title)
    }
}
