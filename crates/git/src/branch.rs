//! Branch operations: list, create, delete, checkout, and merge (regular or
//! squash) - plus a non-destructive conflict check.
//!
//! The ADE's merge flow ("merge the winner") lives here: after comparing runs,
//! the chosen run's branch merges back into the project's base. Before offering
//! that, the UI can call [`would_conflict`] to warn without touching the tree.

use std::path::Path;

use crate::run::{git, git_capture, Error};

/// One branch of the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    /// True for the currently checked-out branch.
    pub head: bool,
    /// The upstream tracking ref, if set (e.g. `origin/main`).
    pub upstream: Option<String>,
}

/// The result of a [`merge`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergeOutcome {
    /// Nothing to do - already contains the branch.
    UpToDate,
    /// Fast-forwarded (no merge commit).
    FastForward,
    /// A real merge commit was created.
    Merged,
    /// The merge stopped with conflicts in these paths (working tree left in the
    /// conflicted state; the caller decides whether to resolve or abort).
    Conflicts(Vec<String>),
}

/// List local branches, newest-committed first.
pub fn branches(repo: &Path) -> Result<Vec<Branch>, Error> {
    // %(HEAD) is "*" for the current branch, space otherwise.
    let out = git(
        repo,
        &[
            "branch",
            "--sort=-committerdate",
            "--format=%(HEAD)\t%(refname:short)\t%(upstream:short)",
        ],
    )?;
    Ok(parse_branches(&out))
}

/// Create branch `name` at `start` (default `HEAD`) without checking it out.
pub fn create(repo: &Path, name: &str, start: Option<&str>) -> Result<(), Error> {
    let start = start.unwrap_or("HEAD");
    git(repo, &["branch", name, start])?;
    Ok(())
}

/// Delete branch `name`. With `force`, deletes even if unmerged (`-D`).
pub fn delete(repo: &Path, name: &str, force: bool) -> Result<(), Error> {
    let flag = if force { "-D" } else { "-d" };
    git(repo, &["branch", flag, name])?;
    Ok(())
}

/// Check out branch `name` in `repo`.
pub fn checkout(repo: &Path, name: &str) -> Result<(), Error> {
    git(repo, &["checkout", name])?;
    Ok(())
}

/// The merge-base commit of `a` and `b`, if any.
pub fn merge_base(repo: &Path, a: &str, b: &str) -> Result<Option<String>, Error> {
    let out = git_capture(repo, &["merge-base", a, b])?;
    if out.success {
        let sha = out.stdout.trim().to_string();
        Ok((!sha.is_empty()).then_some(sha))
    } else {
        Ok(None)
    }
}

/// Non-destructively test whether merging `theirs` into `ours` would conflict,
/// returning the conflicted paths. Uses `git merge-tree`, which computes the
/// merge in memory and touches neither the index nor the working tree.
pub fn would_conflict(repo: &Path, ours: &str, theirs: &str) -> Result<Vec<String>, Error> {
    let out = git_capture(
        repo,
        &["merge-tree", "--write-tree", "--name-only", ours, theirs],
    )?;
    if out.success {
        return Ok(Vec::new());
    }
    // On conflict, exit is 1 and stdout is: <tree-oid>\n<conflicted paths...>
    // (an informational message block may follow a blank line). Take the file
    // list between the first line and the first blank line.
    let mut paths = Vec::new();
    for line in out.stdout.lines().skip(1) {
        if line.trim().is_empty() {
            break;
        }
        paths.push(line.trim().to_string());
    }
    Ok(paths)
}

/// Merge `branch` into the current branch of `repo`. Never returns `Err` for a
/// conflict - that is a [`MergeOutcome::Conflicts`]. `Err` is reserved for git
/// being unavailable or a bad invocation.
pub fn merge(repo: &Path, branch: &str) -> Result<MergeOutcome, Error> {
    let out = git_capture(repo, &["merge", "--no-edit", branch])?;
    let text = format!("{}\n{}", out.stdout, out.stderr);
    if out.success {
        if text.contains("Already up to date") {
            Ok(MergeOutcome::UpToDate)
        } else if text.contains("Fast-forward") {
            Ok(MergeOutcome::FastForward)
        } else {
            Ok(MergeOutcome::Merged)
        }
    } else {
        Ok(MergeOutcome::Conflicts(conflicted_paths(repo)?))
    }
}

/// Squash-merge `branch` into the current branch of `repo`: `git merge
/// --squash` stages `branch`'s combined diff into the index without creating
/// a commit or moving `HEAD`, then - on a clean stage - this makes one commit
/// of it (`message`, defaulting to `"Squash <branch>"`). Note that unlike
/// [`merge`], the presence of "Fast-forward" in git's output is *not* a
/// terminal outcome here: `--squash` prints it whenever the underlying
/// computation was trivial, but still leaves the result uncommitted, so this
/// function only ever treats "Already up to date" as needing no commit.
///
/// Reports conflicts the same way [`merge`] does (a [`MergeOutcome::Conflicts`]
/// rather than an `Err`), but recovery differs: a squash merge never records
/// `MERGE_HEAD`, so [`abort_merge`] cannot undo it - use
/// [`abort_squash_merge`] instead.
pub fn merge_squash(
    repo: &Path,
    branch: &str,
    message: Option<&str>,
) -> Result<MergeOutcome, Error> {
    let out = git_capture(repo, &["merge", "--squash", branch])?;
    if !out.success {
        return Ok(MergeOutcome::Conflicts(conflicted_paths(repo)?));
    }
    let text = format!("{}\n{}", out.stdout, out.stderr);
    if text.contains("Already up to date") {
        return Ok(MergeOutcome::UpToDate);
    }
    let default_message = format!("Squash {branch}");
    let message = message.unwrap_or(&default_message);
    git(
        repo,
        &[
            "-c",
            "user.name=Asylum",
            "-c",
            "user.email=asylum@localhost",
            "commit",
            "-m",
            message,
        ],
    )?;
    Ok(MergeOutcome::Merged)
}

/// Abort an in-progress conflicted merge, restoring the pre-merge state.
pub fn abort_merge(repo: &Path) -> Result<(), Error> {
    git(repo, &["merge", "--abort"])?;
    Ok(())
}

/// Recover from a conflicted (or otherwise uncommitted) [`merge_squash`]. A
/// squash merge never records `MERGE_HEAD`, so `git merge --abort` refuses it
/// ("There is no merge to abort") - reset the index and working tree back to
/// `HEAD` instead, the same restoration `git merge --abort` performs once its
/// `MERGE_HEAD` check passes. Since a squash merge never moves `HEAD`,
/// resetting to it is always the correct, fully-clean recovery.
pub fn abort_squash_merge(repo: &Path) -> Result<(), Error> {
    git(repo, &["reset", "--merge", "HEAD"])?;
    Ok(())
}

/// The conflicted (unmerged) paths left in the index by a [`merge`] or
/// [`merge_squash`] that stopped with conflicts.
fn conflicted_paths(repo: &Path) -> Result<Vec<String>, Error> {
    let status = git_capture(repo, &["diff", "--name-only", "--diff-filter=U"])?;
    Ok(status
        .stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// Commit every tracked and untracked change in a completed run worktree.
/// Returns false when the worktree is already clean. The local identity is
/// scoped to this invocation and never mutates user git configuration.
pub fn commit_all(repo: &Path, message: &str) -> Result<bool, Error> {
    if crate::status::status(repo)?.is_empty() {
        return Ok(false);
    }
    git(repo, &["add", "--all"])?;
    git(
        repo,
        &[
            "-c",
            "user.name=Asylum",
            "-c",
            "user.email=asylum@localhost",
            "commit",
            "-m",
            message,
        ],
    )?;
    Ok(true)
}

fn parse_branches(out: &str) -> Vec<Branch> {
    out.lines()
        .filter_map(|line| {
            let mut cols = line.splitn(3, '\t');
            let head = cols.next()? == "*";
            let name = cols.next()?.trim().to_string();
            if name.is_empty() {
                return None;
            }
            let upstream = cols
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            Some(Branch {
                name,
                head,
                upstream,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "../tests/branch.rs"]
mod tests;
