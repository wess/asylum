//! Drain queued work from the companion and control surfaces into live runs.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use gpui::{Context, Window};

use store::{RunStatus, TaskStatus};

use super::NoticeTone;
use crate::state::{now, Root};

/// Outcome of draining one queued unit of work that did not succeed. Determines
/// whether the store row is failed terminally or retried with backoff.
enum DrainFail {
    /// The request can never succeed as written (malformed, unknown target);
    /// record a terminal failure without retrying.
    Permanent(String),
    /// A temporary condition (no live run yet, a busy worktree); retry later.
    Transient(String),
}

impl Root {
    /// Deliver any control requests an agent queued through the control surface.
    /// `spawn` starts a helper run on the same task; `check` runs verification in
    /// the run's worktree. Each is claimed before it runs and its outcome is
    /// recorded (succeeded / retried with backoff / failed), so a failure is
    /// never silently represented as success. Rows stranded `running` by a crash
    /// are recovered first. Mirrors [`Self::drain_followups`].
    pub fn drain_control_requests(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = now();
        let _ = self.db.recover_stale_control_requests(now);
        let claimed = self.db.claim_control_requests(now).unwrap_or_default();
        for request in claimed {
            let id = request.id;
            match self.execute_control_request(&request, window, cx) {
                Ok(()) => {
                    let _ = self.db.complete_control_request(id, now);
                }
                Err(DrainFail::Permanent(msg)) => {
                    let _ = self.db.fail_control_request_permanent(id, now, &msg);
                    self.push_notice(NoticeTone::Warning, "Control request failed", msg);
                }
                Err(DrainFail::Transient(msg)) => {
                    let will_retry = self.db.fail_control_request(id, now, &msg).unwrap_or(false);
                    if !will_retry {
                        self.push_notice(NoticeTone::Warning, "Control request gave up", msg);
                    }
                }
            }
        }
    }

    /// Perform one claimed control request. A malformed request (missing agent,
    /// unknown agent/kind, no run) is a [`DrainFail::Permanent`] failure that
    /// will never succeed; a git/pty side-effect that fails is
    /// [`DrainFail::Transient`] and retried with backoff.
    fn execute_control_request(
        &mut self,
        request: &store::ControlRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), DrainFail> {
        match request.kind.as_str() {
            "spawn" => {
                let agent = json_str(&request.payload, "agent").ok_or_else(|| {
                    DrainFail::Permanent("spawn request is missing an agent".into())
                })?;
                if agent::registry::resolve(&agent, &self.settings.custom_agents).is_none() {
                    return Err(DrainFail::Permanent(format!(
                        "{agent} is not in the configured catalog"
                    )));
                }
                let prompt = json_str(&request.payload, "prompt");
                self.spawn_helper_run(request.task_id, &agent, prompt, window, cx)
                    .map_err(DrainFail::Transient)
            }
            "check" => {
                let run_id = request
                    .run_id
                    .ok_or_else(|| DrainFail::Permanent("check request has no run".into()))?;
                self.run_checks_for(run_id, cx);
                Ok(())
            }
            other => Err(DrainFail::Permanent(format!(
                "unknown control request kind: {other}"
            ))),
        }
    }

    /// Create and queue a helper run for `task_id` on `agent`, in a fresh
    /// worktree, optionally with a one-shot `prompt` override. Reuses the normal
    /// launch queue so parallel limits still apply.
    fn spawn_helper_run(
        &mut self,
        task_id: i64,
        agent: &str,
        prompt: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let task = self.db.task(task_id).map_err(|e| e.to_string())?;
        let project = self
            .db
            .project(task.project_id)
            .map_err(|e| e.to_string())?;
        if agent::registry::resolve(agent, &self.settings.custom_agents).is_none() {
            return Err(format!("{agent} is not in the configured catalog"));
        }
        // A unique branch/worktree so a helper never collides with a sibling.
        let existing = self.db.runs(task_id).map(|r| r.len()).unwrap_or(0);
        let slug = agent::plan::slugify(&task.title);
        let base_name = if slug.is_empty() {
            format!("task-{task_id}")
        } else {
            format!("{slug}-{task_id}")
        };
        let branch = format!("asylum/{base_name}-{agent}-h{existing}");
        let worktree_path = format!(
            "{}/{base_name}-{agent}-h{existing}",
            self.settings.worktree_dir
        );

        let project_config = config::load_project(Path::new(&project.path)).0;
        let base = project_config
            .base_branch
            .as_deref()
            .unwrap_or(&project.base_branch)
            .to_string();
        let worktree = git::worktree::create(
            Path::new(&project.path),
            &worktree_path,
            Some(&branch),
            Some(&base),
        )
        .map_err(|e| e.to_string())?;
        // A helper run has no preparing UI to cancel from, but the deadline
        // still protects the drain loop from a hung setup command.
        let cancel = Arc::new(AtomicBool::new(false));
        let report = crate::prepare::run(
            &worktree,
            &project_config,
            &cancel,
            crate::prepare::DEFAULT_TIMEOUT,
        );
        if let Some(message) = crate::prepare::failure_message(&report) {
            return Err(message);
        }

        let run = self
            .db
            .create_run(task_id, agent, &worktree.to_string_lossy(), &branch)
            .map_err(|e| e.to_string())?;
        self.inherit_task_notes(task_id, run.id);
        let created = self.run_event("worktree_created", run.id);
        self.dispatch_event(created, cx);
        if let Some(prompt) = prompt.filter(|p| !p.trim().is_empty()) {
            let _ = self.db.queue_run_with_prompt(run.id, &prompt);
        }
        let _ = self.db.record_event(
            "run_spawned",
            Some(task_id),
            Some(run.id),
            &format!("{{\"agent\":\"{agent}\"}}"),
            now(),
        );
        if !matches!(self.db.task(task_id), Ok(t) if t.status == TaskStatus::Running) {
            let _ = self.db.set_task_status(task_id, TaskStatus::Running, now());
        }
        self.launch_queued(window, cx);
        Ok(())
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

    /// Deliver any follow-ups queued from the mobile companion. Each is claimed,
    /// sent to an active run of its task (a live stdin agent, else a fresh
    /// attempt), and recorded as delivered. If no run can take it yet the row is
    /// retried with backoff rather than silently dropped, and only after
    /// exhausting attempts becomes a durable `failed` row. Crash-stranded rows
    /// are recovered first.
    pub fn drain_followups(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = now();
        let _ = self.db.recover_stale_followups(now);
        let claimed = self.db.claim_followups(now).unwrap_or_default();
        for followup in claimed {
            let id = followup.id;
            match self.deliver_followup(&followup, window, cx) {
                Ok(()) => {
                    let _ = self.db.complete_followup(id, now);
                }
                Err(DrainFail::Permanent(msg)) => {
                    let _ = self.db.fail_followup_permanent(id, now, &msg);
                    self.push_notice(NoticeTone::Warning, "Mobile follow-up not delivered", msg);
                }
                Err(DrainFail::Transient(msg)) => {
                    let will_retry = self.db.fail_followup(id, now, &msg).unwrap_or(false);
                    if !will_retry {
                        self.push_notice(NoticeTone::Warning, "Mobile follow-up gave up", msg);
                    }
                }
            }
        }
    }

    /// Deliver one claimed follow-up to a run of its task. No run to take it yet
    /// is a [`DrainFail::Transient`] condition (a run may start soon); a delivery
    /// error from the run is likewise transient and retried with backoff.
    fn deliver_followup(
        &mut self,
        followup: &store::Followup,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), DrainFail> {
        let runs = self.db.runs(followup.task_id).unwrap_or_default();
        // Prefer a running run (a live agent can take it now), else the most
        // recent run (a finished run starts a new attempt in its worktree).
        let target = runs
            .iter()
            .rev()
            .find(|run| run.status == RunStatus::Running)
            .or_else(|| runs.last());
        let Some(run) = target else {
            return Err(DrainFail::Transient(
                "no run available to deliver the follow-up yet".into(),
            ));
        };
        let run_id = run.id;
        self.send_followup(run_id, followup.message.clone(), window, cx)
            .map_err(DrainFail::Transient)
    }
}

/// Pull a top-level string field from a JSON object body (control-request
/// payloads). `None` when absent, non-string, or the body does not parse.
fn json_str(body: &str, key: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get(key)?
        .as_str()
        .map(str::to_string)
}
