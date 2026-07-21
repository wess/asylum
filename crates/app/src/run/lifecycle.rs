//! Terminal run transitions: finish, timeout, cancel, and retry.

use std::path::Path;
use std::time::Duration;

use gpui::{Context, Entity, Window};
use libsinclair::termview::TermView;

use store::{RunStatus, TaskStatus};

use super::{persist, redact_and_cap, terminal_text, NoticeTone};
use crate::state::{now, Root};

impl Root {
    pub(super) fn finish_run(
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
        let task_id = self.db.run(run_id).ok().map(|run| run.task_id);
        // A terminal transition flushes the full transcript, redacted and capped
        // to the storage budget like every lazy snapshot.
        let output = redact_and_cap(&terminal_text(term, cx));
        tracing::info!(run_id, ?code, "run finished");
        match code {
            Some(0) => {
                // The run's changes are deliberately left uncommitted in the
                // worktree so the Review surface's per-hunk staging stays live
                // for a successful run. The accepted subset (or the whole tree)
                // is committed onto the branch only at merge/PR time
                // (`finalize_worktree`) — see `ready_to_merge` and
                // `create_pr_for_run`.
                if let Err(error) = self.db.finish_run_with_output(run_id, 0, &output, now()) {
                    self.push_error("Could not finish run", error.to_string());
                    self.shutdown_run_term(run_id);
                    return;
                }
                let _ = self.db.set_run_activity(run_id, Some("done"));
                let _ = self.db.record_event(
                    "run_finished",
                    task_id,
                    Some(run_id),
                    "{\"code\":0}",
                    now(),
                );
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
                    self.shutdown_run_term(run_id);
                    return;
                }
                let _ = self.db.set_run_activity(run_id, None);
                let _ = self.db.record_event(
                    "run_failed",
                    task_id,
                    Some(run_id),
                    &format!("{{\"code\":{code}}}"),
                    now(),
                );
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
        // Fire plugin triggers for this terminal transition, off the UI thread.
        // `run_finished` covers every completion (a `when` filter narrows it);
        // a success also yields `diff_ready`, and any failure also `run_failed`.
        let success = code == Some(0);
        let status = if success { "success" } else { "failure" };
        let mut finished = self.run_event("run_finished", run_id).status(status);
        if let Some(code) = code {
            finished = finished.code(code);
        }
        self.dispatch_event(finished, cx);
        if success {
            let diff = self.run_event("diff_ready", run_id).status("success");
            self.dispatch_event(diff, cx);
        } else {
            let mut failed = self.run_event("run_failed", run_id).status("failure");
            if let Some(code) = code {
                failed = failed.code(code);
            }
            self.dispatch_event(failed, cx);
        }
        self.refresh_task_for_run(run_id);
        self.shutdown_run_term(run_id);
        self.run_saved_at.remove(&run_id);
        self.run_save_failed.remove(&run_id);
        self.launch_needed = true;
        cx.notify();
    }

    pub(super) fn arm_timeout(&self, run_id: i64, window: &Window, cx: &mut Context<Self>) {
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
            .map(|term| redact_and_cap(&terminal_text(term, cx)))
            .unwrap_or_default();
        self.shutdown_run_term(run_id);
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
        let finished = self.run_event("run_finished", run_id).status("failure");
        self.dispatch_event(finished, cx);
        let failed = self.run_event("run_failed", run_id).status("failure");
        self.dispatch_event(failed, cx);
        self.refresh_task_for_run(run_id);
        self.launch_needed = true;
    }

    /// Discard a run's live terminal and deterministically end its agent
    /// process group. Dropping the `TermView` entity is not enough on its own:
    /// render-tree clones keep the pty session (and with it the child) alive
    /// for an unpredictable stretch, so the group recorded at spawn is
    /// signalled directly — SIGHUP now, SIGKILL after a grace period. Also the
    /// bookkeeping sweep for runs that already exited: their pidfile is
    /// consumed and the long-dead group id is never reused as a target because
    /// the entry leaves the map here.
    pub fn shutdown_run_term(&mut self, run_id: i64) {
        self.run_terms.remove(&run_id);
        // Every terminal transition funnels through here, so drop the run's
        // lazy-persistence bookkeeping alongside its terminal and pidfile.
        persist::forget(run_id);
        if let Some(pidfile) = self.run_pidfiles.remove(&run_id) {
            crate::reap::terminate(&pidfile);
        }
    }

    /// Drain the live pid bookkeeping for the quit path, which ends every
    /// remaining agent process group synchronously before the process exits.
    pub fn take_run_pidfiles(&mut self) -> Vec<std::path::PathBuf> {
        self.run_pidfiles.drain().map(|(_, path)| path).collect()
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
        // The live transcript is redacted and capped; the stored fallback was
        // already prepared the same way by earlier snapshots.
        let output = self
            .run_terms
            .get(&run_id)
            .map(|term| redact_and_cap(&terminal_text(term, cx)))
            .unwrap_or(run.output);
        self.shutdown_run_term(run_id);
        if let Err(error) = self.db.cancel_run_with_output(run_id, &output, now()) {
            self.push_error("Could not cancel run", error.to_string());
            return;
        }
        tracing::info!(run_id, "run cancelled");
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
        self.shutdown_run_term(run_id);
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

    pub(super) fn refresh_task_for_run(&mut self, run_id: i64) {
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
}
