//! The low-level `git` invocation helper shared by every submodule.

use std::path::Path;
use std::process::Command;

/// A git operation failure. Either the process could not be launched, or git
/// exited non-zero and we carry its trimmed stderr.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// `git` could not be spawned at all (not installed, permission, …).
    #[error("could not run git: {0}")]
    Spawn(String),
    /// git ran but exited non-zero; the payload is its (trimmed) stderr.
    #[error("git: {0}")]
    Git(String),
}

/// Run `git` in `dir` with `args`, returning stdout on success or git's stderr
/// (trimmed) as [`Error::Git`].
pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<String, Error> {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| Error::Spawn(e.to_string()))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg = err.trim();
        Err(Error::Git(if msg.is_empty() {
            "git command failed".to_string()
        } else {
            msg.to_string()
        }))
    }
}

/// The full result of a `git` run when the caller needs the exit status *and*
/// output regardless of success (e.g. a merge that conflicts is not an error to
/// us — it's an outcome to inspect).
pub(crate) struct Output {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Run `git` capturing status + both streams, without treating non-zero as an
/// error. Returns [`Error::Spawn`] only when git could not be launched.
pub(crate) fn git_capture(dir: &Path, args: &[&str]) -> Result<Output, Error> {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| Error::Spawn(e.to_string()))?;
    Ok(Output {
        success: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Initialize a git repository in `dir` and seed it with an empty initial
/// commit, so it immediately has a `HEAD` (and thus supports worktrees). A
/// throwaway identity is set inline so the commit succeeds even when the user
/// has no global git identity.
pub fn init_repo(dir: &Path) -> Result<(), Error> {
    git(dir, &["init"])?;
    git(
        dir,
        &[
            "-c",
            "user.name=Asylum",
            "-c",
            "user.email=asylum@localhost",
            "commit",
            "--allow-empty",
            "-m",
            "Initial commit",
        ],
    )?;
    Ok(())
}

/// True when `dir` is inside a git work tree.
pub fn is_repo(dir: &Path) -> bool {
    git(dir, &["rev-parse", "--is-inside-work-tree"])
        .map(|s| s.trim() == "true")
        .unwrap_or(false)
}

/// The absolute path of the repository's top-level directory containing `dir`.
pub fn toplevel(dir: &Path) -> Result<std::path::PathBuf, Error> {
    Ok(std::path::PathBuf::from(
        git(dir, &["rev-parse", "--show-toplevel"])?.trim(),
    ))
}

/// The current branch name (`HEAD` symbolic ref), or `None` when detached.
pub fn current_branch(dir: &Path) -> Option<String> {
    let out = git(dir, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok()?;
    let name = out.trim();
    (!name.is_empty()).then(|| name.to_string())
}

#[cfg(test)]
#[path = "../tests/run.rs"]
mod tests;
