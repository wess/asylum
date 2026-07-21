//! The run under review: diffs, annotations, and the fleet's run snapshots.

use gpui::{Context, Window};
use libsinclair::termview::TermView;

use store::RunStatus;

use crate::state::{now, Root};
use crate::workspace::TabKind;

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

    /// The project's local branches (for the branch list in the review view).
    pub fn branches(&self) -> Vec<git::Branch> {
        let dir = std::path::PathBuf::from(self.project_path());
        git::branch::branches(&dir).unwrap_or_default()
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
        // A successful run's commit is deferred until merge, so its committed
        // diff is empty while the work sits in the worktree - an empty
        // since-fork result must fall back to the worktree-vs-HEAD diff the
        // staging surface operates on.
        match git::diff::since_fork(&wt, &base) {
            Ok(files) if !files.is_empty() => files,
            _ => git::diff::against(&wt, "HEAD").unwrap_or_default(),
        }
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
}
