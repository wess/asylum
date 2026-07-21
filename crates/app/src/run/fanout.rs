//! Fan a task out across agents: worktree creation, project setup, and
//! recording of the resulting runs.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gpui::{Context, Window};

use store::TaskStatus;

use super::NoticeTone;
use crate::state::{now, Root};

enum FanoutResult {
    Ready {
        plan: agent::plan::RunPlan,
        worktree: PathBuf,
    },
    SetupFailed {
        plan: agent::plan::RunPlan,
        worktree: PathBuf,
        report: crate::prepare::SetupReport,
    },
    SetupCancelled {
        plan: agent::plan::RunPlan,
        worktree: PathBuf,
        report: crate::prepare::SetupReport,
    },
    WorktreeFailed {
        agent: String,
        error: String,
    },
}

impl Root {
    pub fn toggle_agent(&mut self, id: &str) {
        if self.fanout.iter().any(|agent| agent == id) {
            self.fanout.retain(|agent| agent != id);
        } else {
            self.fanout.push(id.to_string());
        }
    }

    /// Apply a named fan-out layout: replace the current selection with the
    /// preset's agents (those that resolve in the catalog). A no-op for an
    /// unknown name.
    pub fn apply_layout(&mut self, name: &str) {
        let Some(layout) = self.settings.layout(name) else {
            return;
        };
        self.fanout = layout
            .agents
            .iter()
            .filter(|id| agent::registry::resolve(id, &self.settings.custom_agents).is_some())
            .cloned()
            .collect();
    }

    /// The names of the configured fan-out layouts, for the composer picker.
    pub fn layout_names(&self) -> Vec<String> {
        self.settings
            .layouts
            .iter()
            .map(|l| l.name.clone())
            .collect()
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

    /// The agent reports paired with their last CLI probe, for the Settings
    /// surface's Agents section.
    pub fn agent_rows(&self) -> Vec<crate::settings::AgentRow> {
        self.agent_reports()
            .into_iter()
            .map(|(agent, report)| crate::settings::AgentRow {
                test: self.agent_tests.get(&agent.id).cloned(),
                agent,
                report,
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
        // Worktrees start from the user's chosen ref when set, else the base.
        let base = if self.start_ref.trim().is_empty() {
            project_config
                .base_branch
                .as_deref()
                .unwrap_or(&project.base_branch)
                .to_string()
        } else {
            self.start_ref.trim().to_string()
        };
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
        // A shared cancel flag the preparing UI can flip: the background job
        // polls it to kill the running setup command and stop preparing the
        // rest (see `crate::prepare` and `cancel_fanout`).
        let cancel = Arc::new(AtomicBool::new(false));
        self.fanout_cancel = Some(cancel.clone());
        self.fanout_in_progress = true;
        let timeout = crate::prepare::DEFAULT_TIMEOUT;
        let job = cx.background_executor().spawn(async move {
            let mut results = Vec::new();
            for (plan, agent_name) in prepared {
                // Stop before creating another worktree once cancelled — a
                // command mid-flight is killed inside `crate::prepare::run`.
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let worktree = match git::worktree::create(
                    &work_repo,
                    &plan.worktree,
                    Some(&plan.branch),
                    Some(&base),
                ) {
                    Ok(path) => path,
                    Err(error) => {
                        results.push(FanoutResult::WorktreeFailed {
                            agent: agent_name,
                            error: error.to_string(),
                        });
                        continue;
                    }
                };
                let report = crate::prepare::run(&worktree, &setup, &cancel, timeout);
                results.push(match report.outcome() {
                    crate::prepare::SetupOutcome::Ok => FanoutResult::Ready { plan, worktree },
                    crate::prepare::SetupOutcome::Cancelled => FanoutResult::SetupCancelled {
                        plan,
                        worktree,
                        report,
                    },
                    crate::prepare::SetupOutcome::Failed => FanoutResult::SetupFailed {
                        plan,
                        worktree,
                        report,
                    },
                });
            }
            results
        });
        let Some(handle) = window.window_handle().downcast::<Root>() else {
            self.fanout_in_progress = false;
            self.fanout_cancel = None;
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
        // Whether the user cancelled this batch: an already-prepared run is then
        // recorded as cancelled rather than launched, so "Cancel" stops the
        // whole preparation instead of letting finished worktrees run on.
        let batch_cancelled = self
            .fanout_cancel
            .take()
            .is_some_and(|flag| flag.load(Ordering::Relaxed));
        let mut queued = Vec::new();
        let mut recorded = Vec::new();
        for result in results {
            match result {
                FanoutResult::WorktreeFailed { agent, error } => {
                    self.push_error(
                        format!("Could not create {agent} worktree"),
                        format!(
                            "{error}. Fix the repository or branch state, then dispatch again."
                        ),
                    );
                }
                FanoutResult::Ready { plan, worktree } => {
                    let Some(run_id) = self.record_run(task_id, &plan, &worktree, cx) else {
                        continue;
                    };
                    recorded.push(run_id);
                    if batch_cancelled {
                        if let Err(error) = self.db.cancel_run_with_output(run_id, "", now()) {
                            self.push_error("Could not cancel run", error.to_string());
                        }
                    } else {
                        queued.push(run_id);
                    }
                }
                FanoutResult::SetupFailed {
                    plan,
                    worktree,
                    report,
                } => {
                    let Some(run_id) = self.record_run(task_id, &plan, &worktree, cx) else {
                        continue;
                    };
                    recorded.push(run_id);
                    // The per-command transcript is the run's stored output; the
                    // plain-language headline (which command, exit code, output
                    // tail) is the error, mirroring how launch failures record.
                    let transcript = crate::prepare::transcript(&report);
                    let message = crate::prepare::failure_message(&report)
                        .unwrap_or_else(|| "Project setup failed.".to_string());
                    let stored = format!(
                        "{message}\n\nInspect {} and retry after fixing setup.",
                        worktree.display()
                    );
                    if let Err(error) = self.db.fail_run(run_id, &stored, &transcript, now()) {
                        self.push_error("Could not record setup failure", error.to_string());
                    }
                    self.push_error(format!("Setup failed for {}", plan.agent), message);
                }
                FanoutResult::SetupCancelled {
                    plan,
                    worktree,
                    report,
                } => {
                    let Some(run_id) = self.record_run(task_id, &plan, &worktree, cx) else {
                        continue;
                    };
                    recorded.push(run_id);
                    let transcript = crate::prepare::transcript(&report);
                    if let Err(error) = self.db.cancel_run_with_output(run_id, &transcript, now()) {
                        self.push_error("Could not cancel run", error.to_string());
                    }
                }
            }
        }
        self.selected_run_id = queued.first().or(recorded.first()).copied();
        if recorded.is_empty() {
            return;
        }
        if batch_cancelled {
            self.push_notice(
                NoticeTone::Info,
                "Setup cancelled",
                "Preparation stopped. Prepared worktrees were preserved and can be retried.",
            );
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

    /// Record a run row for a prepared worktree and fire `worktree_created`.
    /// Returns the new run id, or `None` (with an error surfaced) when the store
    /// write fails. Shared by the ready / failed / cancelled fan-out outcomes.
    fn record_run(
        &mut self,
        task_id: i64,
        plan: &agent::plan::RunPlan,
        worktree: &Path,
        cx: &mut Context<Self>,
    ) -> Option<i64> {
        match self.db.create_run(
            task_id,
            &plan.agent,
            &worktree.to_string_lossy(),
            &plan.branch,
        ) {
            Ok(run) => {
                self.inherit_task_notes(task_id, run.id);
                let created = self.run_event("worktree_created", run.id);
                self.dispatch_event(created, cx);
                Some(run.id)
            }
            Err(error) => {
                self.push_error(
                    "Could not record run",
                    format!("{error}. The worktree remains at {}.", worktree.display()),
                );
                None
            }
        }
    }

    /// Cancel an in-flight fan-out preparation: flip the shared flag so the
    /// background job kills the running setup command's process group and stops
    /// preparing further worktrees. Recording of the (partial) runs happens when
    /// the job returns through [`Self::finish_fanout`].
    pub fn cancel_fanout(&mut self) {
        if let Some(flag) = &self.fanout_cancel {
            flag.store(true, Ordering::Relaxed);
            self.push_notice(
                NoticeTone::Info,
                "Cancelling setup",
                "Stopping the current setup command and the rest of preparation.",
            );
        }
    }
}
