//! Git operations for the Agent Development Environment.
//!
//! Every task in the ADE runs in its own isolated git worktree - fan one prompt
//! across N agents, each editing its own tree, then compare and merge the
//! winner. This crate is the pure, I/O-only git layer that makes that possible:
//! it shells out to the `git` binary and returns typed results. No gpui, no
//! async, no persistence - the higher layers (`agent`, `store`, `app`) build on
//! it.
//!
//! Submodules:
//! - [`run`] - the low-level `git` invocation helper and [`Error`].
//! - [`worktree`] - create / list / remove worktrees.
//! - [`status`] - working-tree status (porcelain v2).
//! - [`diff`] - unified-diff capture and parsing into a reviewable model.
//! - [`stage`] - per-hunk index staging (accept a subset of a run's changes).

pub mod branch;
pub mod diff;
mod run;
pub mod stage;
pub mod status;
pub mod worktree;

pub use branch::{merge_base, would_conflict, Branch, MergeOutcome};
pub use diff::{DiffFile, DiffHunk, DiffLine, FileStatus, LineKind};
pub use run::{current_branch, init_repo, is_repo, toplevel, Error};
pub use stage::{commit_staged, has_staged_subset};
pub use status::{Entry, StatusKind};
pub use worktree::Worktree;
