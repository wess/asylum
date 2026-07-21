//! GitHub and Linear: issue browsing, issue worktrees, and PR creation.

use store::RunStatus;

use crate::state::{now, Root};
use crate::workspace::TabKind;

/// A git branch name derived from a Linear issue: its identifier plus a short
/// slug of the title (e.g. `eng-123-add-login`).
fn linear_branch(issue: &linear::Issue) -> String {
    let ident = issue.identifier.to_lowercase().replace(['/', ' '], "-");
    let title: String = issue
        .title
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let title = title
        .split('-')
        .filter(|part| !part.is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join("-");
    if title.is_empty() {
        ident
    } else {
        format!("{ident}-{title}")
    }
}

impl Root {
    /// Create a worktree + task from a GitHub issue. Best effort on the
    /// worktree; the task is always created.
    pub fn create_worktree_from_issue(&mut self, issue: &github::Issue) {
        let Some(pid) = self.project_id else {
            return;
        };
        let Ok(project) = self.db.project(pid) else {
            return;
        };
        let repo = std::path::PathBuf::from(&project.path);
        let branch = github::issue_branch(issue);
        let worktree = format!(
            "{}/{branch}",
            self.settings.worktree_dir.trim_end_matches('/')
        );
        let absolute = match git::worktree::create(&repo, &worktree, Some(&branch), None) {
            Ok(path) => path,
            Err(error) => {
                self.push_error("Could not open issue worktree", error.to_string());
                return;
            }
        };
        let prompt = format!("Resolve GitHub issue #{}: {}", issue.number, issue.title);
        if let Ok(task) = self.db.create_task(pid, &issue.title, &prompt, now()) {
            self.task_id = Some(task.id);
            self.push_notice(
                crate::run::NoticeTone::Success,
                "Issue worktree ready",
                absolute.display().to_string(),
            );
            self.open_kind(TabKind::Tasks);
        }
    }

    /// Open a pull request for a run's branch (open a PR from the IDE).
    pub fn create_pr_for_run(&mut self, run_id: i64) {
        let Ok(run) = self.db.run(run_id) else {
            self.push_error("Run unavailable", "The selected run no longer exists.");
            return;
        };
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
        if run.status != RunStatus::Succeeded {
            self.push_error(
                "Run is not ready",
                "Only a successful run can be opened as a pull request.",
            );
            return;
        }
        if self.checking_runs.contains(&run_id) {
            self.push_error(
                "Checks are still running",
                "Wait for verification to finish before opening the pull request.",
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
                "Fix or rerun failed checks before opening the pull request.",
            );
            return;
        }
        if results.is_empty() || checks::overall(&results) == checks::Status::Skipped {
            self.push_notice(
                crate::run::NoticeTone::Warning,
                "Run is not fully verified",
                "No executable checks passed. Review the diff and terminal output carefully.",
            );
        }
        // Commit the accepted work onto the run's branch before it is pushed:
        // the staged subset when the review curated one, else the whole
        // worktree. Deferred from run-finish and mirrored from the merge path so
        // a PR carries exactly the reviewed changes. A clean worktree is a no-op.
        if let Err(error) = crate::run::finalize_worktree(
            std::path::Path::new(&run.worktree),
            &format!("Complete task: {}", task.title),
        ) {
            self.push_error(
                "Could not finalize run",
                format!("{error}. Open the run terminal, fix the git state, then retry."),
            );
            return;
        }
        let repo = std::path::PathBuf::from(&project.path);
        let base = config::load_project(&repo)
            .0
            .base_branch
            .unwrap_or(project.base_branch);
        match github::create_pr(&repo, &task.title, &task.prompt, &base, &run.branch) {
            Ok(url) => {
                self.reference_run_notes(run_id, &notes::Reference::pullrequest(&url));
                self.push_notice(
                    crate::run::NoticeTone::Success,
                    "Pull request opened",
                    url.clone(),
                );
                self.push_notification("run_finished", "PR opened", &url, Some(run_id));
            }
            Err(error) => {
                self.push_error("Could not open pull request", error.to_string());
                self.push_notification("attention", "PR failed", &error.to_string(), Some(run_id));
            }
        }
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
        self.load_linear();
    }

    /// Load Linear issues when an API token is configured. Absent token leaves
    /// the list empty (the surface then shows how to set one); an API error is
    /// captured for display, never a crash.
    pub fn load_linear(&mut self) {
        let token = self.settings.linear_token.trim();
        if token.is_empty() {
            self.linear_issues.clear();
            return;
        }
        match linear::Client::new(token).issues() {
            Ok(issues) => self.linear_issues = issues,
            Err(e) => {
                if self.integration_error.is_none() {
                    self.integration_error = Some(format!("Linear: {e}"));
                }
            }
        }
    }

    /// Create a worktree + task from a Linear issue, mirroring the GitHub flow.
    pub fn create_worktree_from_linear_issue(&mut self, issue: &linear::Issue) {
        let Some(pid) = self.project_id else {
            return;
        };
        let Ok(project) = self.db.project(pid) else {
            return;
        };
        let repo = std::path::PathBuf::from(&project.path);
        let branch = linear_branch(issue);
        let worktree = format!(
            "{}/{branch}",
            self.settings.worktree_dir.trim_end_matches('/')
        );
        let absolute = match git::worktree::create(&repo, &worktree, Some(&branch), None) {
            Ok(path) => path,
            Err(error) => {
                self.push_error("Could not open issue worktree", error.to_string());
                return;
            }
        };
        let prompt = format!("Resolve Linear issue {}: {}", issue.identifier, issue.title);
        if let Ok(task) = self.db.create_task(pid, &issue.title, &prompt, now()) {
            self.task_id = Some(task.id);
            self.push_notice(
                crate::run::NoticeTone::Success,
                "Issue worktree ready",
                absolute.display().to_string(),
            );
            self.open_kind(TabKind::Tasks);
        }
    }
}
