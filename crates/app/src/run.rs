//! Durable run orchestration and safety checks for the desktop app.

mod check;
mod cleanup;
mod dispatch;
mod drain;
mod fanout;
mod launch;
mod lifecycle;
mod merge;
mod persist;

pub(crate) use launch::redact_and_cap;
pub use launch::terminal_text;
pub(crate) use merge::finalize_worktree;

use gpui::Context;

use crate::state::Root;
use crate::workspace::TabKind;

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
    SquashMerge(i64),
    DeleteProject(i64),
    DeleteTask(i64),
    DeleteNote(String),
    DeleteAccount(i64),
    ArchiveTask(i64),
    RemoveWorktree {
        run_id: i64,
        force: bool,
    },
    CleanupTask(i64),
    /// Enable a fully-trusted (process-runtime) plugin. Carries the trust
    /// disclosure so the confirm bar can restate exactly what will run and with
    /// what authority before the enable is committed.
    EnablePlugin {
        id: String,
        name: String,
        disclosure: String,
    },
}

impl ConfirmAction {
    pub fn title(&self) -> String {
        match self {
            Self::Merge(_) => "Merge this run?".into(),
            Self::SquashMerge(_) => "Squash-merge this run?".into(),
            Self::DeleteProject(_) => "Remove this project from Asylum?".into(),
            Self::DeleteTask(_) => "Delete this task?".into(),
            Self::DeleteNote(_) => "Delete this note?".into(),
            Self::DeleteAccount(_) => "Delete this account?".into(),
            Self::ArchiveTask(_) => "Archive this task?".into(),
            Self::RemoveWorktree { force: true, .. } => "Discard this dirty worktree?".into(),
            Self::RemoveWorktree { force: false, .. } => "Remove this worktree?".into(),
            Self::CleanupTask(_) => "Clean up finished worktrees?".into(),
            Self::EnablePlugin { name, .. } => format!("Enable {name}?"),
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Merge(_) => {
                "The selected branch will be merged into the project's base branch.".into()
            }
            Self::SquashMerge(_) => {
                "The selected branch's changes will be combined into one new commit on the project's base branch."
                    .into()
            }
            Self::DeleteProject(_) => {
                "Tasks and run history will be deleted. Repository files are kept.".into()
            }
            Self::DeleteTask(_) => {
                "Run history and clean worktrees will be deleted. Dirty worktrees block deletion."
                    .into()
            }
            Self::DeleteNote(_) => {
                "The Markdown file and its task/run attachments will be deleted.".into()
            }
            Self::DeleteAccount(_) => {
                "Saved usage history will also be deleted. Provider credentials are not changed."
                    .into()
            }
            Self::ArchiveTask(_) => "The task will move out of the active workflow.".into(),
            Self::RemoveWorktree { force: true, .. } => {
                "Uncommitted changes in this worktree will be permanently deleted.".into()
            }
            Self::RemoveWorktree { force: false, .. } => {
                "The worktree directory will be removed. The branch is kept.".into()
            }
            Self::CleanupTask(_) => {
                "Only clean, finished worktrees are removed. Dirty worktrees are preserved.".into()
            }
            Self::EnablePlugin { disclosure, .. } => disclosure.clone(),
        }
    }
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

    pub fn open_run_terminal(&mut self, run_id: i64) {
        self.select_run(run_id);
        let id = self.next_tab_id();
        self.workspace.open(id, TabKind::Run(run_id));
    }

    pub fn confirm_action(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.confirm.take() else {
            return;
        };
        match action {
            ConfirmAction::Merge(id) => self.merge_run_now(id, cx),
            ConfirmAction::SquashMerge(id) => self.squash_merge_run_now(id, cx),
            ConfirmAction::DeleteProject(id) => self.remove_project(id),
            ConfirmAction::DeleteTask(id) => self.delete_task(id),
            ConfirmAction::DeleteNote(path) => self.delete_note(&path, cx),
            ConfirmAction::DeleteAccount(id) => match self.db.delete_account(id) {
                Ok(()) => self.push_notice(
                    NoticeTone::Success,
                    "Account deleted",
                    "Another account for this provider is selected automatically when available.",
                ),
                Err(error) => self.push_error("Could not delete account", error.to_string()),
            },
            ConfirmAction::ArchiveTask(id) => self.archive_task(id),
            ConfirmAction::RemoveWorktree { run_id, force } => {
                self.remove_worktree_now(run_id, force, cx)
            }
            ConfirmAction::CleanupTask(id) => self.cleanup_task_now(id),
            ConfirmAction::EnablePlugin { id, .. } => self.enable_plugin_now(&id, cx),
        }
    }
}
