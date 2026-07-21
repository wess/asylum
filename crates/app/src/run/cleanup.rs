//! Remove finished worktrees and clean up a task's leftovers.

use std::path::Path;

use gpui::Context;

use store::RunStatus;

use super::{ConfirmAction, NoticeTone};
use crate::state::Root;

impl Root {
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

    pub(super) fn remove_worktree_now(&mut self, run_id: i64, force: bool, cx: &mut Context<Self>) {
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
            Ok(()) => {
                self.push_notice(
                    NoticeTone::Success,
                    "Worktree removed",
                    "The run history and branch were kept.",
                );
                let removed = self.run_event("worktree_removed", run_id);
                self.dispatch_event(removed, cx);
            }
            Err(error) => self.push_error("Could not remove worktree", error.to_string()),
        }
    }

    pub(super) fn cleanup_task_now(&mut self, task_id: i64) {
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
        let repo = Path::new(&project.path);
        let mut removed = 0;
        let mut preserved = 0;
        let mut branches_deleted = 0;
        for run in self.db.runs(task_id).unwrap_or_default() {
            if !run.status.is_terminal() {
                continue;
            }
            let worktree = Path::new(&run.worktree);
            let worktree_gone = if !worktree.exists() {
                true
            } else {
                match git::status::status(worktree) {
                    Ok(entries) if entries.is_empty() => {
                        let ok = git::worktree::remove(repo, worktree, false).is_ok();
                        if ok {
                            removed += 1;
                        }
                        ok
                    }
                    _ => {
                        preserved += 1;
                        false
                    }
                }
            };
            // Only try once no worktree still has the branch checked out.
            // `-d` (never `-D`) refuses anything not fully merged into the
            // base branch, so a losing run's branch is silently left alone.
            if worktree_gone && git::branch::delete(repo, &run.branch, false).is_ok() {
                branches_deleted += 1;
            }
        }
        let _ = git::worktree::prune(repo);
        self.push_notice(
            NoticeTone::Info,
            "Worktree cleanup finished",
            format!(
                "Removed {removed} clean worktree(s); preserved {preserved} dirty or \
                 unreadable worktree(s); deleted {branches_deleted} merged branch(es)."
            ),
        );
    }
}
