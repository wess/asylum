//! The low-level `git` invocation helper shared by every submodule.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

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

/// Build a `git` invocation in `dir` with `args`, pinned to the `C` locale so
/// output this crate pattern-matches on (e.g. [`crate::branch::merge`]'s
/// "Fast-forward") stays in English regardless of the caller's system locale.
fn command(dir: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir)
        .args(args)
        .env("LC_ALL", "C")
        .env("LANG", "C");
    cmd
}

/// Run `git` in `dir` with `args`, returning stdout on success or git's stderr
/// (trimmed) as [`Error::Git`].
pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<String, Error> {
    let out = command(dir, args)
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
/// us - it's an outcome to inspect).
pub(crate) struct Output {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Run `git` capturing status + both streams, without treating non-zero as an
/// error. Returns [`Error::Spawn`] only when git could not be launched.
pub(crate) fn git_capture(dir: &Path, args: &[&str]) -> Result<Output, Error> {
    let out = command(dir, args)
        .output()
        .map_err(|e| Error::Spawn(e.to_string()))?;
    Ok(Output {
        success: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Run `git` with `input` fed to its stdin (e.g. `git apply` reading a patch),
/// capturing status + both streams like [`git_capture`]. The stdin pipe is
/// closed before waiting, so a git that reads to EOF never deadlocks against a
/// child that is also filling its stdout.
pub(crate) fn git_stdin(dir: &Path, args: &[&str], input: &str) -> Result<Output, Error> {
    let mut child = command(dir, args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Spawn(e.to_string()))?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::Spawn("git stdin was not captured".to_string()))?;
        stdin
            .write_all(input.as_bytes())
            .map_err(|e| Error::Spawn(e.to_string()))?;
        // `stdin` drops here, closing the pipe so git sees EOF.
    }
    let out = child
        .wait_with_output()
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
