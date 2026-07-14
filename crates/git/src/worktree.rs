//! Git worktree operations: create, list, and remove worktrees.
//!
//! The ADE gives every agent task its own worktree so parallel agents never
//! collide on the index or working tree. Creating a task allocates a worktree
//! (usually on a fresh branch); finishing or discarding it removes the worktree.

use std::path::{Path, PathBuf};

use crate::run::{git, Error};

/// One entry from `git worktree list --porcelain`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    /// Absolute path of the worktree's working directory.
    pub path: PathBuf,
    /// The checked-out branch (`refs/heads/…` stripped), if any.
    pub branch: Option<String>,
    /// The checked-out commit, if reported.
    pub head: Option<String>,
    /// True for the primary worktree (the original clone).
    pub primary: bool,
}

/// Resolve `path` against `base`: absolute paths are used as-is, relative ones
/// are joined onto `base` (the repository directory).
fn resolve(base: &Path, path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else {
        let absolute_base = if base.is_absolute() {
            base.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(base)
        };
        absolute_base.join(p)
    }
}

/// Create a worktree at `path` (relative to `repo` when not absolute). With a
/// non-empty `branch`, a new branch of that name is created from `start`
/// (defaulting to `HEAD`); without one, git derives a branch from the final
/// path component. Returns the absolute worktree path.
pub fn create(
    repo: &Path,
    path: &str,
    branch: Option<&str>,
    start: Option<&str>,
) -> Result<PathBuf, Error> {
    let abs = resolve(repo, path);
    let abs_str = abs.to_string_lossy().into_owned();
    let start = start.unwrap_or("HEAD");
    match branch {
        Some(b) if !b.is_empty() => {
            git(repo, &["worktree", "add", "-b", b, &abs_str, start])?;
        }
        _ => {
            git(repo, &["worktree", "add", &abs_str, start])?;
        }
    }
    Ok(abs)
}

/// List every worktree of the repository containing `repo`.
pub fn list(repo: &Path) -> Result<Vec<Worktree>, Error> {
    let out = git(repo, &["worktree", "list", "--porcelain"])?;
    Ok(parse_list(&out))
}

/// Remove the worktree at `path`. With `force`, removes even a dirty worktree.
pub fn remove(repo: &Path, path: &Path, force: bool) -> Result<(), Error> {
    let p = path.to_string_lossy().into_owned();
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(&p);
    git(repo, &args)?;
    Ok(())
}

/// Prune administrative records for worktrees whose directories are gone.
pub fn prune(repo: &Path) -> Result<(), Error> {
    git(repo, &["worktree", "prune"])?;
    Ok(())
}

/// Parse `git worktree list --porcelain` output. Records are blank-line
/// separated; the first record is the primary worktree.
fn parse_list(out: &str) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut path: Option<PathBuf> = None;
    let mut branch: Option<String> = None;
    let mut head: Option<String> = None;
    let mut first = true;

    let mut flush = |path: &mut Option<PathBuf>,
                     branch: &mut Option<String>,
                     head: &mut Option<String>,
                     first: &mut bool| {
        if let Some(p) = path.take() {
            worktrees.push(Worktree {
                path: p,
                branch: branch.take(),
                head: head.take(),
                primary: std::mem::replace(first, false),
            });
        }
    };

    for line in out.lines() {
        if line.is_empty() {
            flush(&mut path, &mut branch, &mut head, &mut first);
            continue;
        }
        if let Some(rest) = line.strip_prefix("worktree ") {
            flush(&mut path, &mut branch, &mut head, &mut first);
            path = Some(PathBuf::from(rest));
        } else if let Some(rest) = line.strip_prefix("branch ") {
            branch = Some(rest.strip_prefix("refs/heads/").unwrap_or(rest).to_string());
        } else if let Some(rest) = line.strip_prefix("HEAD ") {
            head = Some(rest.to_string());
        }
    }
    flush(&mut path, &mut branch, &mut head, &mut first);
    worktrees
}

#[cfg(test)]
#[path = "../tests/worktree.rs"]
mod tests;
