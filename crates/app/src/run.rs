//! Durable run orchestration and safety checks for the desktop app.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{App, AppContext as _, Context, Entity, Window};
use libsinclair::terminal::{Event, SessionOptions};
use libsinclair::termview::{TermOptions, TermView};
use store::{RunStatus, TaskStatus};

use crate::state::{now, Root};
use crate::workspace::TabKind;

enum FanoutResult {
    Ready {
        plan: agent::plan::RunPlan,
        worktree: PathBuf,
    },
    SetupFailed {
        plan: agent::plan::RunPlan,
        worktree: PathBuf,
        error: String,
    },
    WorktreeFailed {
        agent: String,
        error: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeTone {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub id: u64,
    pub tone: NoticeTone,
    pub title: String,
    pub message: String,
}

impl Notice {
    pub fn error(id: u64, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id,
            tone: NoticeTone::Error,
            title: title.into(),
            message: message.into(),
        }
    }

    pub fn warning(id: u64, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id,
            tone: NoticeTone::Warning,
            title: title.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    Merge(i64),
    DeleteProject(i64),
    DeleteTask(i64),
    DeleteNote(String),
    ArchiveTask(i64),
    RemoveWorktree { run_id: i64, force: bool },
    CleanupTask(i64),
}

impl ConfirmAction {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Merge(_) => "Merge this run?",
            Self::DeleteProject(_) => "Remove this project from Asylum?",
            Self::DeleteTask(_) => "Delete this task?",
            Self::DeleteNote(_) => "Delete this note?",
            Self::ArchiveTask(_) => "Archive this task?",
            Self::RemoveWorktree { force: true, .. } => "Discard this dirty worktree?",
            Self::RemoveWorktree { force: false, .. } => "Remove this worktree?",
            Self::CleanupTask(_) => "Clean up finished worktrees?",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::Merge(_) => "The selected branch will be merged into the project's base branch.",
            Self::DeleteProject(_) => {
                "Tasks and run history will be deleted. Repository files are kept."
            }
            Self::DeleteTask(_) => {
                "Run history and clean worktrees will be deleted. Dirty worktrees block deletion."
            }
            Self::DeleteNote(_) => {
                "The Markdown file and its task/run attachments will be deleted."
            }
            Self::ArchiveTask(_) => "The task will move out of the active workflow.",
            Self::RemoveWorktree { force: true, .. } => {
                "Uncommitted changes in this worktree will be permanently deleted."
            }
            Self::RemoveWorktree { force: false, .. } => {
                "The worktree directory will be removed. The branch is kept."
            }
            Self::CleanupTask(_) => {
                "Only clean, finished worktrees are removed. Dirty worktrees are preserved."
            }
        }
    }
}

pub fn terminal_text(term: &Entity<TermView>, cx: &App) -> String {
    term.read(cx).session().with_term(|terminal| {
        terminal
            .text_lines()
            .into_iter()
            .map(|(_, line, _)| line)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end()
            .to_string()
    })
}

impl Root {
    pub fn push_notice(
        &mut self,
        tone: NoticeTone,
        title: impl Into<String>,
        message: impl Into<String>,
    ) {
        let id = self.next_notice_id;
        self.next_notice_id += 1;
        self.notices.push(Notice {
            id,
            tone,
            title: title.into(),
            message: message.into(),
        });
        if self.notices.len() > 4 {
            self.notices.remove(0);
        }
    }

    pub fn push_error(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.push_notice(NoticeTone::Error, title, message);
    }

    pub fn dismiss_notice(&mut self, id: u64) {
        self.notices.retain(|notice| notice.id != id);
    }

    pub fn select_run(&mut self, id: i64) {
        let Ok(run) = self.db.run(id) else {
            self.push_error("Run unavailable", "The selected run no longer exists.");
            return;
        };
        self.task_id = Some(run.task_id);
        self.selected_run_id = Some(id);
    }

    pub fn run_check_results(&self, run_id: i64) -> Vec<checks::CheckResult> {
        self.db
            .run_checks(run_id)
            .unwrap_or_default()
            .into_iter()
            .map(|result| checks::CheckResult {
                id: result.id,
                status: match result.status.as_str() {
                    "pass" => checks::Status::Pass,
                    "fail" => checks::Status::Fail,
                    _ => checks::Status::Skipped,
                },
                summary: result.summary,
                duration_ms: result.duration_ms as u128,
            })
            .collect()
    }

    pub fn run_checks(&mut self, cx: &mut Context<Self>) {
        let Some(run_id) = self.current_run_id() else {
            self.push_error("No run selected", "Select a run before starting checks.");
            return;
        };
        self.run_checks_for(run_id, cx);
    }

    fn run_checks_for(&mut self, run_id: i64, cx: &mut Context<Self>) {
        if !self.checking_runs.insert(run_id) {
            return;
        }
        let Ok(run) = self.db.run(run_id) else {
            self.checking_runs.remove(&run_id);
            self.push_error("Run unavailable", "The selected run no longer exists.");
            return;
        };
        let worktree = PathBuf::from(run.worktree);
        if !worktree.exists() {
            self.checking_runs.remove(&run_id);
            self.push_error(
                "Worktree missing",
                "Restore or retry the run before starting checks.",
            );
            return;
        }
        let job = cx.background_executor().spawn(async move {
            let detected = checks::detect(&worktree);
            checks::run_all(&worktree, &detected)
        });
        cx.spawn(async move |root, cx| {
            let results = job.await;
            root.update(cx, |root, cx| {
                let stored: Vec<store::RunCheck> = results
                    .iter()
                    .map(|result| store::RunCheck {
                        run_id,
                        id: result.id.clone(),
                        status: result.status.as_str().to_string(),
                        summary: result.summary.clone(),
                        duration_ms: result.duration_ms.min(u64::MAX as u128) as u64,
                    })
                    .collect();
                root.checking_runs.remove(&run_id);
                if let Err(error) = root.db.replace_run_checks(run_id, &stored) {
                    root.push_error("Could not save checks", error.to_string());
                } else {
                    let summary = if results.is_empty() {
                        "No checks detected".to_string()
                    } else {
                        format!(
                            "{} check(s), overall {}",
                            results.len(),
                            checks::overall(&results).as_str()
                        )
                    };
                    root.reference_run_notes(
                        run_id,
                        &notes::Reference::checks(run_id, &summary),
                    );
                }
                if results.is_empty() {
                    root.push_notice(
                        NoticeTone::Warning,
                        "No checks detected",
                        "Add project test, lint, or type-check configuration before merging high-risk changes.",
                    );
                } else if checks::overall(&results) == checks::Status::Fail {
                    root.push_error(
                        "Checks failed",
                        "Open Review for the failed command summary, then send fixes to the selected agent.",
                    );
                } else {
                    root.push_notice(
                        NoticeTone::Success,
                        "Checks finished",
                        "Verification results were saved with this run.",
                    );
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle_agent(&mut self, id: &str) {
        if self.fanout.iter().any(|agent| agent == id) {
            self.fanout.retain(|agent| agent != id);
        } else {
            self.fanout.push(id.to_string());
        }
    }

    pub fn agent_reports(&self) -> Vec<(agent::registry::Agent, agent::doctor::Report)> {
        let verified = self.db.successful_agents().unwrap_or_default();
        agent::registry::catalog(&self.settings.custom_agents)
            .into_iter()
            .map(|mut agent| {
                if let Some(program) = self
                    .settings
                    .agents
                    .get(&agent.id)
                    .and_then(|prefs| prefs.program.clone())
                {
                    agent.program = program;
                }
                let mut report = agent::doctor::inspect(&agent);
                report.verified = verified.contains(&agent.id);
                (agent, report)
            })
            .collect()
    }

    pub fn choose_recommended_agent(&mut self) {
        if !self.fanout.is_empty() {
            return;
        }
        if let Some((agent, _)) = self
            .agent_reports()
            .into_iter()
            .find(|(_, report)| report.ready())
        {
            self.fanout.push(agent.id);
        }
    }

    pub fn run_fanout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fanout_in_progress {
            self.push_error(
                "Runs are already being prepared",
                "Wait for project setup to finish.",
            );
            return;
        }
        let Some(task_id) = self.task_id else {
            self.push_error(
                "No task selected",
                "Create or select a task before running agents.",
            );
            return;
        };
        let Ok(task) = self.db.task(task_id) else {
            self.push_error("Task unavailable", "The selected task could not be loaded.");
            return;
        };
        let Ok(project) = self.db.project(task.project_id) else {
            self.push_error(
                "Project unavailable",
                "The task's project could not be loaded.",
            );
            return;
        };
        if !self.db.runs(task_id).unwrap_or_default().is_empty() {
            self.push_error(
                "Task already has runs",
                "Retry a specific run or create a new task instead of dispatching this task twice.",
            );
            return;
        }
        if self.fanout.is_empty() {
            self.choose_recommended_agent();
        }
        if self.fanout.is_empty() {
            self.push_error(
                "No ready agents",
                "Install an agent CLI or configure its program in Settings, then select it here.",
            );
            return;
        }

        let repo = PathBuf::from(&project.path);
        let (project_config, diagnostics) = config::load_project(&repo);
        for diagnostic in diagnostics {
            self.push_error("Project settings", diagnostic.message);
        }
        let base = project_config
            .base_branch
            .as_deref()
            .unwrap_or(&project.base_branch);
        let plans = agent::plan::fanout(
            task_id,
            &task.title,
            &self.fanout,
            &self.settings.worktree_dir,
        );
        let mut prepared = Vec::new();
        for plan in plans {
            let Some(agent) = agent::registry::resolve(&plan.agent, &self.settings.custom_agents)
            else {
                self.push_error(
                    "Unknown agent",
                    format!("{} is not in the configured catalog.", plan.agent),
                );
                continue;
            };
            let prefs = self.settings.agents.get(&plan.agent);
            let spec = agent::command::build(&agent, prefs, &task.prompt, &project.path);
            if agent::doctor::find_program(&spec.program).is_none() {
                self.push_error(
                    format!("{} is not ready", agent.name),
                    format!("{} was not found on PATH. Install it or set a program override in Settings.", spec.program),
                );
                continue;
            }
            prepared.push((plan, agent.name));
        }
        if prepared.is_empty() {
            return;
        }
        let base = base.to_string();
        let setup = project_config.clone();
        let work_repo = repo.clone();
        self.fanout_in_progress = true;
        let job = cx.background_executor().spawn(async move {
            prepared
                .into_iter()
                .map(|(plan, agent_name)| {
                    let worktree = match git::worktree::create(
                        &work_repo,
                        &plan.worktree,
                        Some(&plan.branch),
                        Some(&base),
                    ) {
                        Ok(path) => path,
                        Err(error) => {
                            return FanoutResult::WorktreeFailed {
                                agent: agent_name,
                                error: error.to_string(),
                            };
                        }
                    };
                    match run_setup(&worktree, &setup) {
                        Ok(()) => FanoutResult::Ready { plan, worktree },
                        Err(error) => FanoutResult::SetupFailed {
                            plan,
                            worktree,
                            error,
                        },
                    }
                })
                .collect::<Vec<_>>()
        });
        let Some(handle) = window.window_handle().downcast::<Root>() else {
            self.fanout_in_progress = false;
            self.push_error(
                "Could not prepare runs",
                "The application window is unavailable.",
            );
            return;
        };
        cx.spawn(async move |_root, cx| {
            let results = job.await;
            let _ = handle.update(cx, |root, window, cx| {
                root.finish_fanout(task_id, results, window, cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn finish_fanout(
        &mut self,
        task_id: i64,
        results: Vec<FanoutResult>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.fanout_in_progress = false;
        let mut queued = Vec::new();
        let mut recorded = Vec::new();
        for result in results {
            let (plan, worktree, setup_error) = match result {
                FanoutResult::Ready { plan, worktree } => (plan, worktree, None),
                FanoutResult::SetupFailed {
                    plan,
                    worktree,
                    error,
                } => (plan, worktree, Some(error)),
                FanoutResult::WorktreeFailed { agent, error } => {
                    self.push_error(
                        format!("Could not create {agent} worktree"),
                        format!(
                            "{error}. Fix the repository or branch state, then dispatch again."
                        ),
                    );
                    continue;
                }
            };
            match self.db.create_run(
                task_id,
                &plan.agent,
                &worktree.to_string_lossy(),
                &plan.branch,
            ) {
                Ok(run) => {
                    recorded.push(run.id);
                    self.inherit_task_notes(task_id, run.id);
                    if let Some(error) = setup_error {
                        let message = format!(
                            "Project setup failed: {error}. Inspect {} and retry after fixing setup.",
                            worktree.display()
                        );
                        if let Err(store_error) = self.db.fail_run(run.id, &message, "", now()) {
                            self.push_error(
                                "Could not record setup failure",
                                store_error.to_string(),
                            );
                        }
                        self.push_error(format!("Setup failed for {}", plan.agent), message);
                    } else {
                        queued.push(run.id);
                    }
                }
                Err(error) => self.push_error(
                    "Could not record run",
                    format!("{error}. The worktree remains at {}.", worktree.display()),
                ),
            }
        }
        self.selected_run_id = queued.first().or(recorded.first()).copied();
        if recorded.is_empty() {
            return;
        }
        let status = if queued.is_empty() {
            TaskStatus::Review
        } else {
            TaskStatus::Running
        };
        if let Err(error) = self.db.set_task_status(task_id, status, now()) {
            self.push_error("Could not update task", error.to_string());
        }
        if !queued.is_empty() {
            self.push_notice(
                NoticeTone::Info,
                "Runs queued",
                format!(
                    "{} agent run(s) are ready and respect the global parallel limit.",
                    queued.len()
                ),
            );
            self.launch_queued(window, cx);
        }
    }

    pub fn launch_queued(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.launch_needed = false;
        let running = match self.db.running_count() {
            Ok(count) => count,
            Err(error) => {
                self.push_error("Could not inspect running agents", error.to_string());
                return;
            }
        };
        let limit = if self.settings.max_parallel_runs == 0 {
            usize::MAX
        } else {
            self.settings.max_parallel_runs as usize
        };
        let capacity = limit.saturating_sub(running);
        if capacity == 0 {
            return;
        }
        let queued = match self.db.queued_runs() {
            Ok(runs) => runs,
            Err(error) => {
                self.push_error("Could not read run queue", error.to_string());
                return;
            }
        };
        for run in queued.into_iter().take(capacity) {
            self.launch_run(run.id, window, cx);
        }
    }

    fn launch_run(&mut self, run_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(run) = self.db.run(run_id) else {
            self.push_error(
                "Run unavailable",
                format!("Queued run {run_id} could not be loaded."),
            );
            return;
        };
        let Ok(task) = self.db.task(run.task_id) else {
            self.fail_launch(run_id, "The task for this run no longer exists.");
            return;
        };
        let Ok(project) = self.db.project(task.project_id) else {
            self.fail_launch(run_id, "The project for this run no longer exists.");
            return;
        };
        let Some(agent) = agent::registry::resolve(&run.agent, &self.settings.custom_agents) else {
            self.fail_launch(run_id, "The configured agent no longer exists.");
            return;
        };
        let mut prompt = run.prompt.clone().unwrap_or(task.prompt);
        prompt.push_str(&self.note_context_for_run(run_id));
        let prefs = self.settings.agents.get(&run.agent);
        let spec = agent::command::build(&agent, prefs, &prompt, &run.worktree);
        if agent::doctor::find_program(&spec.program).is_none() {
            self.fail_launch(run_id, &format!("{} was not found on PATH.", spec.program));
            return;
        }
        let project_config = config::load_project(Path::new(&project.path)).0;
        let term = match make_term(spec.clone(), project_config.env, window, cx) {
            Ok(term) => term,
            Err(error) => {
                self.fail_launch(run_id, &format!("Could not start {}: {error}", agent.name));
                return;
            }
        };
        if let Some(stdin) = spec.stdin {
            let write = term
                .read(cx)
                .session()
                .write(stdin.as_bytes())
                .and_then(|_| term.read(cx).session().write(b"\n"));
            if let Err(error) = write {
                self.fail_launch(
                    run_id,
                    &format!("Could not send the prompt to {}: {error}", agent.name),
                );
                return;
            }
        }
        if let Err(error) = self.db.start_run(run_id, now()) {
            self.push_error("Could not start run", error.to_string());
            self.fail_launch(
                run_id,
                "The run could not be marked as started in the workspace store.",
            );
            return;
        }
        cx.subscribe(&term, move |root, term, event: &Event, cx| match event {
            Event::Wakeup => root.snapshot_run(run_id, &term, cx),
            Event::Exit(code) => root.finish_run(run_id, *code, &term, cx),
            _ => {}
        })
        .detach();
        self.run_terms.insert(run_id, term);
        self.arm_timeout(run_id, window, cx);
    }

    fn fail_launch(&mut self, run_id: i64, message: &str) {
        if let Err(error) = self.db.fail_run(run_id, message, "", now()) {
            self.push_error("Could not record launch failure", error.to_string());
        }
        self.push_error(
            "Agent launch failed",
            format!("{message} Retry after fixing the setup."),
        );
        self.refresh_task_for_run(run_id);
        self.launch_needed = true;
    }

    fn snapshot_run(&mut self, run_id: i64, term: &Entity<TermView>, cx: &App) {
        let second = now();
        if self.run_saved_at.get(&run_id) == Some(&second) {
            return;
        }
        self.run_saved_at.insert(run_id, second);
        match self.db.save_run_output(run_id, &terminal_text(term, cx)) {
            Ok(()) => {
                self.run_save_failed.remove(&run_id);
            }
            Err(error) if self.run_save_failed.insert(run_id) => {
                self.push_error(
                    "Terminal output is not being saved",
                    format!("{error}. The live terminal remains available, but restart recovery is at risk."),
                );
            }
            Err(_) => {}
        }
    }

    fn finish_run(
        &mut self,
        run_id: i64,
        code: Option<i32>,
        term: &Entity<TermView>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .db
            .run(run_id)
            .ok()
            .is_some_and(|run| run.status == RunStatus::Running)
        {
            return;
        }
        let output = terminal_text(term, cx);
        match code {
            Some(0) => {
                let commit = self
                    .db
                    .run(run_id)
                    .and_then(|run| self.db.task(run.task_id).map(|task| (run, task)))
                    .map_err(|error| error.to_string())
                    .and_then(|(run, task)| {
                        git::branch::commit_all(
                            Path::new(&run.worktree),
                            &format!("Complete task: {}", task.title),
                        )
                        .map_err(|error| error.to_string())
                    });
                if let Err(error) = commit {
                    if let Err(store_error) = self.db.fail_run(
                        run_id,
                        &format!(
                            "The run finished but its changes could not be committed: {error}"
                        ),
                        &output,
                        now(),
                    ) {
                        self.push_error("Could not record run failure", store_error.to_string());
                    }
                    self.push_error(
                        "Could not finalize run",
                        format!("{error}. Open the run terminal, fix the git state, then retry."),
                    );
                    self.refresh_task_for_run(run_id);
                    self.run_terms.remove(&run_id);
                    self.run_saved_at.remove(&run_id);
                    self.run_save_failed.remove(&run_id);
                    self.launch_needed = true;
                    cx.notify();
                    return;
                }
                if let Err(error) = self.db.finish_run_with_output(run_id, 0, &output, now()) {
                    self.push_error("Could not finish run", error.to_string());
                    self.run_terms.remove(&run_id);
                    return;
                }
                self.refresh_setup();
                self.push_notice(
                    NoticeTone::Success,
                    "Run ready to review",
                    "Compare its changes and checks before choosing a winner.",
                );
                self.push_notification(
                    "run_finished",
                    "Run ready to review",
                    "Compare its changes and checks.",
                    Some(run_id),
                );
                self.run_checks_for(run_id, cx);
            }
            Some(code) => {
                if let Err(error) = self.db.finish_run_with_output(run_id, code, &output, now()) {
                    self.push_error("Could not finish run", error.to_string());
                    self.run_terms.remove(&run_id);
                    return;
                }
                self.push_notice(
                    NoticeTone::Error,
                    "Run failed",
                    format!("The agent exited with code {code}. Open its terminal output, fix setup, or retry."),
                );
                self.push_notification(
                    "run_failed",
                    "Run failed",
                    &format!("Exit code {code}"),
                    Some(run_id),
                );
            }
            None => {
                if let Err(error) = self.db.fail_run(
                    run_id,
                    "The agent process ended without an exit code.",
                    &output,
                    now(),
                ) {
                    self.push_error("Could not record run failure", error.to_string());
                }
                self.push_error(
                    "Run stopped unexpectedly",
                    "Review the terminal output, then retry the run.",
                );
                self.push_notification(
                    "run_failed",
                    "Run stopped unexpectedly",
                    "Review its terminal output.",
                    Some(run_id),
                );
            }
        }
        self.refresh_task_for_run(run_id);
        self.run_terms.remove(&run_id);
        self.run_saved_at.remove(&run_id);
        self.run_save_failed.remove(&run_id);
        self.launch_needed = true;
        cx.notify();
    }

    fn arm_timeout(&self, run_id: i64, window: &Window, cx: &mut Context<Self>) {
        let minutes = self.settings.run_timeout_minutes;
        if minutes == 0 {
            return;
        }
        let timer = cx
            .background_executor()
            .timer(Duration::from_secs(minutes as u64 * 60));
        let Some(handle) = window.window_handle().downcast::<Root>() else {
            return;
        };
        cx.spawn(async move |_root, cx| {
            timer.await;
            let _ = handle.update(cx, |root, _window, cx| {
                if root
                    .db
                    .run(run_id)
                    .ok()
                    .is_some_and(|run| run.status == RunStatus::Running)
                {
                    root.timeout_run(run_id, cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn timeout_run(&mut self, run_id: i64, cx: &mut Context<Self>) {
        let output = self
            .run_terms
            .get(&run_id)
            .map(|term| terminal_text(term, cx))
            .unwrap_or_default();
        self.run_terms.remove(&run_id);
        let message = format!(
            "The run exceeded the {} minute timeout.",
            self.settings.run_timeout_minutes
        );
        if let Err(error) = self.db.fail_run(run_id, &message, &output, now()) {
            self.push_error("Could not record timeout", error.to_string());
        }
        self.push_error(
            "Run timed out",
            format!("{message} Retry it or increase the timeout in Settings."),
        );
        self.push_notification("run_failed", "Run timed out", &message, Some(run_id));
        self.refresh_task_for_run(run_id);
        self.launch_needed = true;
    }

    pub fn cancel_run(&mut self, run_id: i64, cx: &mut Context<Self>) {
        let Ok(run) = self.db.run(run_id) else {
            self.push_error("Run unavailable", "The selected run no longer exists.");
            return;
        };
        if !matches!(run.status, RunStatus::Queued | RunStatus::Running) {
            self.push_error(
                "Run is not active",
                "Only queued or running work can be cancelled.",
            );
            return;
        }
        let output = self
            .run_terms
            .get(&run_id)
            .map(|term| terminal_text(term, cx))
            .unwrap_or(run.output);
        self.run_terms.remove(&run_id);
        if let Err(error) = self.db.cancel_run_with_output(run_id, &output, now()) {
            self.push_error("Could not cancel run", error.to_string());
            return;
        }
        self.push_notice(
            NoticeTone::Info,
            "Run cancelled",
            "The worktree was preserved and can be retried.",
        );
        self.push_notification(
            "attention",
            "Run cancelled",
            "The worktree was preserved.",
            Some(run_id),
        );
        self.refresh_task_for_run(run_id);
        self.launch_needed = true;
    }

    pub fn retry_run(&mut self, run_id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(run) = self.db.run(run_id) else {
            self.push_error("Run unavailable", "The selected run no longer exists.");
            return;
        };
        if matches!(run.status, RunStatus::Queued | RunStatus::Running) {
            self.push_error("Run is still active", "Cancel it before retrying.");
            return;
        }
        if !Path::new(&run.worktree).exists() {
            self.push_error(
                "Worktree missing",
                "The run cannot be retried because its worktree was removed.",
            );
            return;
        }
        self.run_terms.remove(&run_id);
        if let Err(error) = self.db.queue_run(run_id) {
            self.push_error("Could not retry run", error.to_string());
            return;
        }
        if let Err(error) = self.db.replace_run_checks(run_id, &[]) {
            self.push_error("Could not clear stale checks", error.to_string());
        }
        self.selected_run_id = Some(run_id);
        self.launch_queued(window, cx);
    }

    pub fn send_followup(
        &mut self,
        run_id: i64,
        prompt: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if prompt.trim().is_empty() {
            return Err("The follow-up is empty.".into());
        }
        let run = self.db.run(run_id).map_err(|error| error.to_string())?;
        if run.status == RunStatus::Queued {
            return Err(
                "This run is already queued. Wait for it to launch or cancel it first.".into(),
            );
        }
        if run.status == RunStatus::Running {
            let agent = agent::registry::resolve(&run.agent, &self.settings.custom_agents)
                .ok_or("The configured agent no longer exists.")?;
            if agent.delivery != agent::registry::Delivery::Stdin {
                return Err(
                    "This agent runs one prompt per process. Wait for it to finish, then send the review as another attempt."
                        .into(),
                );
            }
            let term = self
                .run_terms
                .get(&run_id)
                .ok_or("The live terminal is unavailable.")?;
            term.read(cx)
                .session()
                .write(prompt.as_bytes())
                .map_err(|error| error.to_string())?;
            term.read(cx)
                .session()
                .write(b"\n")
                .map_err(|error| error.to_string())?;
            return Ok(());
        }
        self.db
            .queue_run_with_prompt(run_id, &prompt)
            .map_err(|error| error.to_string())?;
        self.db
            .replace_run_checks(run_id, &[])
            .map_err(|error| error.to_string())?;
        self.launch_queued(window, cx);
        Ok(())
    }

    fn refresh_task_for_run(&mut self, run_id: i64) {
        let Ok(run) = self.db.run(run_id) else { return };
        if self
            .db
            .task(run.task_id)
            .ok()
            .is_some_and(|task| matches!(task.status, TaskStatus::Merged | TaskStatus::Archived))
        {
            return;
        }
        let Ok(runs) = self.db.runs(run.task_id) else {
            return;
        };
        let status = if runs
            .iter()
            .any(|run| matches!(run.status, RunStatus::Queued | RunStatus::Running))
        {
            TaskStatus::Running
        } else {
            TaskStatus::Review
        };
        if let Err(error) = self.db.set_task_status(run.task_id, status, now()) {
            self.push_error("Could not update task status", error.to_string());
        }
    }

    pub fn open_run_terminal(&mut self, run_id: i64) {
        self.select_run(run_id);
        let id = self.next_tab_id();
        self.workspace.open(id, TabKind::Run(run_id));
    }

    pub fn request_merge(&mut self, run_id: i64) {
        let Ok(run) = self.db.run(run_id) else {
            self.push_error("Run unavailable", "The selected run no longer exists.");
            return;
        };
        if run.status != RunStatus::Succeeded {
            self.push_error("Run is not ready", "Only a successful run can be merged.");
            return;
        }
        if self.checking_runs.contains(&run_id) {
            self.push_error(
                "Checks are still running",
                "Wait for verification to finish before merging.",
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
                "Fix or explicitly rerun the failed checks before merging this run.",
            );
            return;
        }
        if results.is_empty() || checks::overall(&results) == checks::Status::Skipped {
            self.push_notice(
                NoticeTone::Warning,
                "Run is not fully verified",
                "No executable checks passed. Review the diff and terminal output carefully before confirming.",
            );
        }
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
        let repo = PathBuf::from(&project.path);
        let base = config::load_project(&repo)
            .0
            .base_branch
            .unwrap_or(project.base_branch.clone());
        match base_status(&repo, &self.settings.worktree_dir) {
            Ok(entries) if !entries.is_empty() => {
                self.push_error(
                    "Base worktree is dirty",
                    "Commit, stash, or discard its changes before merging a run.",
                );
                return;
            }
            Err(error) => {
                self.push_error("Could not inspect base worktree", error.to_string());
                return;
            }
            _ => {}
        }
        match git::branch::would_conflict(&repo, &base, &run.branch) {
            Ok(paths) if !paths.is_empty() => {
                self.push_error(
                    "Merge would conflict",
                    format!("{} file(s) conflict. Open a run terminal or create a PR to resolve them safely.", paths.len()),
                );
            }
            Ok(_) => self.confirm = Some(ConfirmAction::Merge(run_id)),
            Err(error) => self.push_error("Could not preflight merge", error.to_string()),
        }
    }

    fn merge_run_now(&mut self, run_id: i64) {
        let Ok(run) = self.db.run(run_id) else {
            self.push_error("Run unavailable", "The selected run no longer exists.");
            return;
        };
        if run.status != RunStatus::Succeeded {
            self.push_error(
                "Run changed before merge",
                "Only a successful, inactive run can be merged.",
            );
            return;
        }
        if self.checking_runs.contains(&run_id) {
            self.push_error(
                "Checks are still running",
                "Wait for verification to finish, then request the merge again.",
            );
            return;
        }
        if self
            .run_check_results(run_id)
            .iter()
            .any(|result| result.status == checks::Status::Fail)
        {
            self.push_error(
                "Checks changed before merge",
                "Fix or rerun failed checks, then request the merge again.",
            );
            return;
        }
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
        let repo = PathBuf::from(&project.path);
        let base = config::load_project(&repo)
            .0
            .base_branch
            .unwrap_or(project.base_branch.clone());
        match base_status(&repo, &self.settings.worktree_dir) {
            Ok(entries) if !entries.is_empty() => {
                self.push_error(
                    "Base worktree changed",
                    "Commit, stash, or discard its changes, then request the merge again.",
                );
                return;
            }
            Err(error) => {
                self.push_error("Could not inspect base worktree", error.to_string());
                return;
            }
            _ => {}
        }
        if let Err(error) = git::branch::checkout(&repo, &base) {
            self.push_error("Could not check out base branch", error.to_string());
            return;
        }
        match git::branch::merge(&repo, &run.branch) {
            Ok(git::MergeOutcome::Conflicts(paths)) => {
                let _ = git::branch::abort_merge(&repo);
                self.push_error(
                    "Merge aborted",
                    format!(
                        "{} unexpected conflict(s) were found. The base worktree was restored.",
                        paths.len()
                    ),
                );
            }
            Ok(_) => {
                if let Err(error) = self.db.set_task_status(task.id, TaskStatus::Merged, now()) {
                    self.push_error("Merged, but task status was not saved", error.to_string());
                }
                self.push_notice(
                    NoticeTone::Success,
                    "Winner merged",
                    format!(
                        "{} is now on {}. Clean up finished worktrees when ready.",
                        run.agent, base
                    ),
                );
                self.push_notification(
                    "run_finished",
                    "Winner merged",
                    &format!("Merged into {base}"),
                    Some(run_id),
                );
            }
            Err(error) => self.push_error("Merge failed", error.to_string()),
        }
    }

    pub fn request_remove_worktree(&mut self, run_id: i64) {
        let Ok(run) = self.db.run(run_id) else {
            self.push_error("Run unavailable", "The selected run no longer exists.");
            return;
        };
        if matches!(run.status, RunStatus::Queued | RunStatus::Running) {
            self.push_error("Run is active", "Cancel it before removing its worktree.");
            return;
        }
        let dirty = match git::status::status(Path::new(&run.worktree)) {
            Ok(entries) => !entries.is_empty(),
            Err(error) => {
                self.push_error(
                    "Could not inspect worktree",
                    format!("{error}. The worktree was preserved."),
                );
                return;
            }
        };
        self.confirm = Some(ConfirmAction::RemoveWorktree {
            run_id,
            force: dirty,
        });
    }

    fn remove_worktree_now(&mut self, run_id: i64, force: bool) {
        let Ok(run) = self.db.run(run_id) else {
            self.push_error("Run unavailable", "The selected run no longer exists.");
            return;
        };
        if matches!(run.status, RunStatus::Queued | RunStatus::Running) {
            self.push_error(
                "Run changed before cleanup",
                "Cancel it before removing its worktree.",
            );
            return;
        }
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
        match git::worktree::remove(Path::new(&project.path), Path::new(&run.worktree), force) {
            Ok(()) => self.push_notice(
                NoticeTone::Success,
                "Worktree removed",
                "The run history and branch were kept.",
            ),
            Err(error) => self.push_error("Could not remove worktree", error.to_string()),
        }
    }

    fn cleanup_task_now(&mut self, task_id: i64) {
        let Ok(task) = self.db.task(task_id) else {
            self.push_error("Task unavailable", "The selected task no longer exists.");
            return;
        };
        let Ok(project) = self.db.project(task.project_id) else {
            self.push_error(
                "Project unavailable",
                "The task's project no longer exists.",
            );
            return;
        };
        let mut removed = 0;
        let mut preserved = 0;
        for run in self.db.runs(task_id).unwrap_or_default() {
            if !run.status.is_terminal() || !Path::new(&run.worktree).exists() {
                continue;
            }
            match git::status::status(Path::new(&run.worktree)) {
                Ok(entries) if entries.is_empty() => {
                    if git::worktree::remove(
                        Path::new(&project.path),
                        Path::new(&run.worktree),
                        false,
                    )
                    .is_ok()
                    {
                        removed += 1;
                    }
                }
                _ => preserved += 1,
            }
        }
        self.push_notice(
            NoticeTone::Info,
            "Worktree cleanup finished",
            format!("Removed {removed} clean worktree(s); preserved {preserved} dirty or unreadable worktree(s)."),
        );
    }

    pub fn confirm_action(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.confirm.take() else {
            return;
        };
        match action {
            ConfirmAction::Merge(id) => self.merge_run_now(id),
            ConfirmAction::DeleteProject(id) => self.remove_project(id),
            ConfirmAction::DeleteTask(id) => self.delete_task(id),
            ConfirmAction::DeleteNote(path) => self.delete_note(&path, cx),
            ConfirmAction::ArchiveTask(id) => self.archive_task(id),
            ConfirmAction::RemoveWorktree { run_id, force } => {
                self.remove_worktree_now(run_id, force)
            }
            ConfirmAction::CleanupTask(id) => self.cleanup_task_now(id),
        }
    }
}

fn run_setup(worktree: &Path, config: &config::ProjectConfig) -> Result<(), String> {
    for command in &config.setup {
        #[cfg(unix)]
        let mut child = std::process::Command::new("sh");
        #[cfg(unix)]
        child.arg("-lc").arg(command);
        #[cfg(windows)]
        let mut child = std::process::Command::new("cmd");
        #[cfg(windows)]
        child.arg("/C").arg(command);
        let output = child
            .current_dir(worktree)
            .envs(&config.env)
            .output()
            .map_err(|error| format!("could not run `{command}`: {error}"))?;
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(format!("`{command}` failed: {}", error.trim()));
        }
    }
    Ok(())
}

fn base_status(repo: &Path, worktree_dir: &str) -> Result<Vec<git::Entry>, git::Error> {
    git::status::status(repo)
        .map(|entries| git::status::excluding_prefix(entries, Path::new(worktree_dir)))
}

fn make_term(
    spec: agent::SpawnSpec,
    env: std::collections::BTreeMap<String, String>,
    window: &mut Window,
    cx: &mut Context<Root>,
) -> std::io::Result<Entity<TermView>> {
    let mut options =
        SessionOptions::command(std::iter::once(spec.program).chain(spec.args).collect());
    options.spawn.cwd = Some(PathBuf::from(spec.cwd));
    options.spawn.env.extend(env);
    let (session, events) = libsinclair::Session::spawn(options)?;
    let session = std::sync::Arc::new(session);
    Ok(cx.new(|cx| TermView::new(session, events, TermOptions::default(), window, cx)))
}
