//! Project and task lifecycle: onboarding a repository, creating and
//! deleting tasks, selection, and the workspace tree.

use gpui::{Context, Window};

use store::{RunStatus, TaskStatus};

use crate::state::{now, Root};
use crate::workspace::TabKind;

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

impl Root {
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
                let created = self.task_event("task_created", task.id).status("created");
                self.dispatch_event(created, cx);
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
            self.shutdown_run_term(run.id);
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
            self.shutdown_run_term(run.id);
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

    /// The filesystem path of the selected project (or the cwd as a fallback).
    pub fn project_path(&self) -> String {
        self.project_id
            .and_then(|id| self.db.project(id).ok())
            .map(|p| p.path)
            .unwrap_or_else(|| ".".to_string())
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
            // Drop stale `.git/worktrees/*` records (left by a crash or an
            // out-of-band removal) before anything else reads the repo.
            let _ = git::worktree::prune(std::path::Path::new(&path));
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
