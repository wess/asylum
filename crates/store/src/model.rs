//! Row types and their status enums.
//!
//! Timestamps are unix seconds (`i64`). Ids are SQLite rowids (`i64`). The
//! status enums round-trip through short lowercase tokens stored as TEXT so the
//! database stays legible and stable across schema dumps.

/// A git repository the user works in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: i64,
    pub name: String,
    /// Absolute path to the repository root.
    pub path: String,
    /// Default branch new task worktrees fork from (e.g. `main`).
    pub base_branch: String,
    pub created_at: i64,
    /// Pinned to the top of the workspace list.
    pub pinned: bool,
    /// When the project was last opened (unix seconds; 0 = never) - drives the
    /// "recent repositories" ordering.
    pub last_opened_at: i64,
}

/// Lifecycle of a unit of work on one of the app's drain queues (follow-ups,
/// control requests). A drainer *claims* a `Pending` row (→ `Running`), performs
/// its side-effect, then records the outcome. A transient failure returns the
/// row to `Pending` with a backoff; an exhausted or permanent failure is
/// `Failed` and preserved for inspection. Success is `Succeeded`. A `Running`
/// row left behind by a crash is recovered back to `Pending`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
}

impl QueueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            QueueStatus::Pending => "pending",
            QueueStatus::Running => "running",
            QueueStatus::Succeeded => "succeeded",
            QueueStatus::Failed => "failed",
        }
    }
    /// Parse a stored token; an unknown value is treated as `Pending` so a bad
    /// row is retried rather than silently dropped.
    pub fn parse(s: &str) -> Self {
        match s {
            "running" => QueueStatus::Running,
            "succeeded" => QueueStatus::Succeeded,
            "failed" => QueueStatus::Failed,
            _ => QueueStatus::Pending,
        }
    }
}

/// A message queued against a task from outside the desktop app (today, the
/// mobile companion). The app claims pending rows and delivers each to an active
/// run so it reaches the agent, recording success or a durable failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Followup {
    pub id: i64,
    pub task_id: i64,
    pub message: String,
    pub source: String,
    /// Where this row is in its delivery lifecycle.
    pub status: QueueStatus,
    /// How many times delivery has been attempted.
    pub attempts: i64,
    /// The most recent failure, kept for UI inspection of a `Failed` row.
    pub last_error: Option<String>,
    pub created_at: i64,
}

/// A request an in-worktree agent (or the CLI) queues to orchestrate the ADE -
/// spawn a helper run, run checks, and so on. The desktop app claims pending
/// rows and performs the git/pty side, mirroring the [`Followup`] queue. Keeps
/// the control surface a pure function over the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlRequest {
    pub id: i64,
    pub task_id: i64,
    /// The run that issued the request, if it came from inside a worktree.
    pub run_id: Option<i64>,
    /// What to do, e.g. `spawn` or `check`.
    pub kind: String,
    /// JSON parameters, interpreted by the app per `kind`.
    pub payload: String,
    pub source: String,
    /// Where this row is in its execution lifecycle.
    pub status: QueueStatus,
    /// How many times execution has been attempted.
    pub attempts: i64,
    /// The most recent failure, kept for UI inspection of a `Failed` row.
    pub last_error: Option<String>,
    pub created_at: i64,
}

/// An append-only record of something that happened in the ADE (a run started,
/// an activity changed, a worktree was created). Streamed to the companion and
/// control clients so a phone or an agent can follow the fleet without polling
/// every table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub id: i64,
    /// Category, e.g. `run_started`, `run_activity`, `run_finished`.
    pub kind: String,
    pub task_id: Option<i64>,
    pub run_id: Option<i64>,
    /// JSON detail payload.
    pub data: String,
    pub created_at: i64,
}

/// A unit of work: a prompt, optionally fanned across several agents. Each
/// agent's attempt is a [`Run`] in its own worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    /// The prompt handed to the agent(s).
    pub prompt: String,
    pub status: TaskStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

/// The lifecycle of a [`Task`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// Composed but not yet dispatched to any agent.
    Draft,
    /// At least one run is active.
    Running,
    /// Runs finished; awaiting the user's review/merge decision.
    Review,
    /// A winning run was merged back to the base branch.
    Merged,
    /// Set aside; no longer active.
    Archived,
}

/// One agent's attempt at a [`Task`], executing in an isolated worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub id: i64,
    pub task_id: i64,
    /// Which agent definition drove this run (e.g. `claude-code`, `codex`).
    pub agent: String,
    /// Absolute path of the run's worktree.
    pub worktree: String,
    /// The branch checked out in the worktree.
    pub branch: String,
    pub status: RunStatus,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    /// Process exit code once the agent finished.
    pub exit_code: Option<i32>,
    /// Persisted terminal transcript, available after the pty is gone.
    pub output: String,
    /// Actionable launch/runtime failure, separate from ordinary non-zero output.
    pub error: Option<String>,
    /// Number of times this worktree has been launched for the task.
    pub attempt: u32,
    /// One-shot prompt override for the next queued attempt.
    pub prompt: Option<String>,
    /// Live semantic activity token (`idle`/`working`/`blocked`/`done`),
    /// classified from the agent's output or self-reported over the control
    /// surface. Ephemeral display state, distinct from the lifecycle `status`;
    /// `None` until first observed.
    pub activity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCheck {
    pub run_id: i64,
    pub id: String,
    pub status: String,
    pub summary: String,
    pub duration_ms: u64,
}

/// Where one project's plain Markdown vault lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteVault {
    pub project_id: i64,
    pub mode: NoteVaultMode,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteVaultMode {
    Private,
    Repository,
}

impl NoteVaultMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Repository => "repository",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "repository" => Self::Repository,
            _ => Self::Private,
        }
    }
}

/// A note attached to a task or run. The Markdown remains the source of truth;
/// this row makes context injection durable across restarts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteAttachment {
    pub id: i64,
    pub project_id: i64,
    pub note_path: String,
    pub task_id: Option<i64>,
    pub run_id: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKind {
    Task,
    Run,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRecord {
    pub kind: SearchKind,
    pub id: i64,
    pub title: String,
    pub detail: String,
}

/// The lifecycle of a [`Run`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    /// Worktree allocated; agent not yet started.
    Queued,
    /// Agent process is live.
    Running,
    /// Agent exited zero.
    Succeeded,
    /// Agent exited non-zero.
    Failed,
    /// User cancelled the run.
    Cancelled,
}

/// Which side of a diff a review [`Annotation`] anchors to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Old,
    New,
}

impl Side {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Old => "old",
            Self::New => "new",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "old" => Self::Old,
            _ => Self::New,
        }
    }
}

/// A review comment anchored to a line of a run's diff - the annotatable
/// diff. Comments are collected and shipped back to the agent as feedback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub id: i64,
    pub run_id: i64,
    pub file: String,
    pub line: u32,
    pub side: Side,
    pub body: String,
    pub resolved: bool,
    pub created_at: i64,
}

/// A provider account (Claude, Codex, …) the user can hot-swap between.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: i64,
    /// Provider key, e.g. `claude` or `codex`.
    pub provider: String,
    /// Human label (email / handle).
    pub label: String,
    /// The active account for its provider.
    pub active: bool,
    pub created_at: i64,
}

/// A usage snapshot for an account: tokens/requests used against a limit, with
/// the rate-limit reset time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    pub id: i64,
    pub account_id: i64,
    pub used: i64,
    pub limit: Option<i64>,
    pub resets_at: Option<i64>,
    pub captured_at: i64,
}

impl Usage {
    /// Fraction of the limit consumed in 0..=1, or `None` when no limit is set.
    pub fn fraction(&self) -> Option<f32> {
        self.limit
            .filter(|l| *l > 0)
            .map(|l| (self.used as f32 / l as f32).min(1.0))
    }
}

/// A notification: an agent finished, a run needs attention, a build failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub id: i64,
    /// Category, e.g. `run_finished`, `attention`, `check_failed`.
    pub kind: String,
    pub title: String,
    pub body: String,
    /// The run this concerns, if any.
    pub run_id: Option<i64>,
    pub read: bool,
    pub created_at: i64,
}

impl TaskStatus {
    /// The stored token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Running => "running",
            Self::Review => "review",
            Self::Merged => "merged",
            Self::Archived => "archived",
        }
    }

    /// Parse a stored token, defaulting to [`TaskStatus::Draft`] on garbage.
    pub fn parse(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "review" => Self::Review,
            "merged" => Self::Merged,
            "archived" => Self::Archived,
            _ => Self::Draft,
        }
    }
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Queued,
        }
    }

    /// True once the run has reached a terminal state.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}
