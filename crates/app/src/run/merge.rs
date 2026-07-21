//! Merge a winning run: preflight safety, merge/squash execution, and the
//! deferred worktree finalize shared with PR creation.

use std::path::{Path, PathBuf};

use gpui::Context;

use store::{RunStatus, TaskStatus};

use super::{ConfirmAction, NoticeTone};
use crate::state::{now, Root};

impl Root {
    pub fn request_merge(&mut self, run_id: i64) {
        if self.preflight_merge(run_id) {
            self.confirm = Some(ConfirmAction::Merge(run_id));
        }
    }

    /// Like [`request_merge`], but queues a squash-merge confirmation instead
    /// of a regular one — same preflight gates, different terminal action
    /// once the user confirms.
    pub fn request_squash_merge(&mut self, run_id: i64) {
        if self.preflight_merge(run_id) {
            self.confirm = Some(ConfirmAction::SquashMerge(run_id));
        }
    }

    /// Preflight shared by [`request_merge`] and [`request_squash_merge`]:
    /// run readiness, checks status, a dirty-base-worktree guard, and a
    /// non-destructive conflict probe. Pushes its own notice/error and
    /// returns `true` only when it is safe to show the merge confirmation.
    fn preflight_merge(&mut self, run_id: i64) -> bool {
        let Ok(run) = self.db.run(run_id) else {
            self.push_error("Run unavailable", "The selected run no longer exists.");
            return false;
        };
        if run.status != RunStatus::Succeeded {
            self.push_error("Run is not ready", "Only a successful run can be merged.");
            return false;
        }
        if self.checking_runs.contains(&run_id) {
            self.push_error(
                "Checks are still running",
                "Wait for verification to finish before merging.",
            );
            return false;
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
            return false;
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
            return false;
        };
        let Ok(project) = self.db.project(task.project_id) else {
            self.push_error(
                "Project unavailable",
                "The task's project no longer exists.",
            );
            return false;
        };
        let repo = PathBuf::from(&project.path);
        let base = config::load_project(&repo)
            .0
            .base_branch
            .unwrap_or(project.base_branch.clone());
        match base_status(&repo, &self.settings.worktree_dir) {
            Ok(entries) if !entries.is_empty() => {
                self.push_error(
                    "Uncommitted changes",
                    "The project has uncommitted changes — commit or stash them before \
                     merging (base worktree is dirty).",
                );
                return false;
            }
            Err(error) => {
                self.push_error("Could not inspect base worktree", error.to_string());
                return false;
            }
            _ => {}
        }
        match git::branch::would_conflict(&repo, &base, &run.branch) {
            Ok(paths) if !paths.is_empty() => {
                self.push_error(
                    "Can't merge automatically",
                    format!(
                        "{} file(s) changed on both sides and need manual resolution — open \
                         a run terminal or create a PR instead (merge conflict).",
                        paths.len()
                    ),
                );
                false
            }
            Ok(_) => true,
            Err(error) => {
                self.push_error("Could not preflight merge", error.to_string());
                false
            }
        }
    }

    pub(super) fn merge_run_now(&mut self, run_id: i64, cx: &mut Context<Self>) {
        let Some((run, repo, base)) = self.ready_to_merge(run_id) else {
            return;
        };
        match git::branch::merge(&repo, &run.branch) {
            Ok(git::MergeOutcome::Conflicts(paths)) => {
                let _ = git::branch::abort_merge(&repo);
                self.push_error(
                    "Merge stopped",
                    format!(
                        "{} file(s) changed on both sides — the merge was cancelled and the \
                         project was left untouched (base worktree restored).",
                        paths.len()
                    ),
                );
            }
            Ok(_) => self.merge_succeeded(run_id, &run, &base, cx),
            Err(error) => self.push_error("Merge failed", error.to_string()),
        }
    }

    /// Squash-merge counterpart to [`merge_run_now`]. A squash merge never
    /// records `MERGE_HEAD`, so a conflict here is recovered with
    /// [`git::branch::abort_squash_merge`] rather than [`git::branch::abort_merge`]
    /// — the latter would simply fail ("There is no merge to abort").
    pub(super) fn squash_merge_run_now(&mut self, run_id: i64, cx: &mut Context<Self>) {
        let Some((run, repo, base)) = self.ready_to_merge(run_id) else {
            return;
        };
        match git::branch::merge_squash(&repo, &run.branch, None) {
            Ok(git::MergeOutcome::Conflicts(paths)) => {
                let _ = git::branch::abort_squash_merge(&repo);
                self.push_error(
                    "Merge stopped",
                    format!(
                        "{} file(s) changed on both sides — the merge was cancelled and the \
                         project was left untouched (base worktree restored).",
                        paths.len()
                    ),
                );
            }
            Ok(_) => self.merge_succeeded(run_id, &run, &base, cx),
            Err(error) => self.push_error("Merge failed", error.to_string()),
        }
    }

    /// Re-validates a run and its base worktree right before merging — state
    /// may have drifted since [`request_merge`]/[`request_squash_merge`]
    /// queued the confirmation — commits the accepted work onto the run's
    /// branch via [`finalize_worktree`] (the staged subset if the review
    /// curated one, else the whole worktree), then checks out the base branch.
    /// Pushes its own error and returns `None` when the merge can no longer
    /// proceed; otherwise returns the run plus the repo path and base branch
    /// name to merge into.
    fn ready_to_merge(&mut self, run_id: i64) -> Option<(store::Run, PathBuf, String)> {
        let Ok(run) = self.db.run(run_id) else {
            self.push_error("Run unavailable", "The selected run no longer exists.");
            return None;
        };
        if run.status != RunStatus::Succeeded {
            self.push_error(
                "Run changed before merge",
                "Only a successful, inactive run can be merged.",
            );
            return None;
        }
        if self.checking_runs.contains(&run_id) {
            self.push_error(
                "Checks are still running",
                "Wait for verification to finish, then request the merge again.",
            );
            return None;
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
            return None;
        }
        let Ok(task) = self.db.task(run.task_id) else {
            self.push_error("Task unavailable", "The run's task no longer exists.");
            return None;
        };
        let Ok(project) = self.db.project(task.project_id) else {
            self.push_error(
                "Project unavailable",
                "The task's project no longer exists.",
            );
            return None;
        };
        let repo = PathBuf::from(&project.path);
        let base = config::load_project(&repo)
            .0
            .base_branch
            .unwrap_or(project.base_branch.clone());
        match base_status(&repo, &self.settings.worktree_dir) {
            Ok(entries) if !entries.is_empty() => {
                self.push_error(
                    "Uncommitted changes",
                    "The project picked up uncommitted changes since the preflight check — \
                     commit or stash them, then request the merge again (base worktree \
                     changed).",
                );
                return None;
            }
            Err(error) => {
                self.push_error("Could not inspect base worktree", error.to_string());
                return None;
            }
            _ => {}
        }
        // Commit the accepted work onto the run's branch before merging: the
        // curated subset when the review staged one, otherwise the whole
        // worktree. Deferred from run-finish so staging stays live until now. A
        // clean worktree (already committed on a prior attempt) is a no-op.
        if let Err(error) = finalize_worktree(
            Path::new(&run.worktree),
            &format!("Complete task: {}", task.title),
        ) {
            self.push_error(
                "Could not finalize run",
                format!("{error}. Open the run terminal, fix the git state, then retry."),
            );
            return None;
        }
        if let Err(error) = git::branch::checkout(&repo, &base) {
            self.push_error("Could not check out base branch", error.to_string());
            return None;
        }
        Some((run, repo, base))
    }

    /// Shared success path for [`merge_run_now`] and [`squash_merge_run_now`]:
    /// mark the task merged and tell the user.
    fn merge_succeeded(
        &mut self,
        run_id: i64,
        run: &store::Run,
        base: &str,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self
            .db
            .set_task_status(run.task_id, TaskStatus::Merged, now())
        {
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
        let merged = self.run_event("task_merged", run_id).status("merged");
        self.dispatch_event(merged, cx);
    }
}

fn base_status(repo: &Path, worktree_dir: &str) -> Result<Vec<git::Entry>, git::Error> {
    git::status::status(repo)
        .map(|entries| git::status::excluding_prefix(entries, Path::new(worktree_dir)))
}

/// Commit a finished run's worktree onto its branch at the moment it is merged
/// or opened as a pull request — the point the finish-time auto-commit was
/// deferred to so the Review surface's per-hunk staging could stay live.
///
/// When the review curated a *subset* (something staged **and** something still
/// unstaged) exactly that subset is committed ([`git::commit_staged`]);
/// otherwise the whole worktree is committed ([`git::branch::commit_all`]),
/// preserving the pre-deferral commit shape. Both underlying calls are no-ops on
/// a clean worktree, so finalize is safe to call unconditionally before every
/// merge/PR — a run whose work was already committed on an earlier attempt, or a
/// retried run, simply commits nothing.
pub(crate) fn finalize_worktree(worktree: &Path, message: &str) -> Result<(), String> {
    if git::has_staged_subset(worktree).map_err(|error| error.to_string())? {
        git::commit_staged(worktree, message).map_err(|error| error.to_string())?;
    } else {
        git::branch::commit_all(worktree, message).map_err(|error| error.to_string())?;
    }
    Ok(())
}
