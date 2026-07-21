//! Per-hunk staging: move a single hunk of a file into or out of the index, so
//! a review can accept a *subset* of a run's changes and carry only that into
//! the eventual merge or PR.
//!
//! The model is `git add -p`'s. The worktree holds the run's changes; the index
//! is the curated selection. [`stage_hunk`] extracts the exact text of one
//! [`DiffHunk`] from a fresh `git diff` and pipes that minimal patch to `git
//! apply --cached`; [`unstage_hunk`] reverse-applies the staged copy of the hunk
//! (`--cached -R`). [`staged`]/[`unstaged`] expose the index-vs-worktree split
//! over the shared diff parser so the surface can render each hunk's staged
//! state, and [`has_staged_subset`]/[`commit_staged`] let the merge flow commit
//! exactly the accepted part.
//!
//! ## Why the patch is *extracted*, not reconstructed
//!
//! Rather than rebuild the unified diff from the parsed [`DiffLine`] model
//! (which drops git's `\ No newline at end of file` marker, the file mode, and —
//! after `str::lines()` — `\r` bytes), a hunk is sliced verbatim from git's own
//! `git diff` output, byte for byte, and reused with its original header. The
//! parsed hunk is only an *index*: it names which file and which `@@ -old_start`
//! to pull. As a result new/deleted files, non-executable/executable modes,
//! CRLF content, and missing trailing newlines all round-trip correctly, because
//! git wrote the bytes.
//!
//! ## Supported forms
//!
//! Added, deleted, and modified files stage per hunk. Individual hunks of a
//! *renamed* file are refused with a clear [`Error::Git`] — the rename headers
//! make a single-hunk slice ambiguous to `git apply --cached` — so stage the
//! file as a whole with [`stage_file`] instead. Every apply is dry-run
//! `--check`ed before it mutates the index, so an unfit patch is a clean error,
//! never a half-applied index.

use std::path::Path;

use crate::diff::{self, DiffFile, DiffHunk, FileStatus};
use crate::run::{git, git_capture, git_stdin, Error};

/// The identity scope used for [`commit_staged`], mirroring the throwaway
/// identity the rest of the crate commits under so a user without a configured
/// git identity is never blocked.
const IDENTITY: [&str; 4] = [
    "-c",
    "user.name=Asylum",
    "-c",
    "user.email=asylum@localhost",
];

/// Stage a single `hunk` of `file` into the index (`git apply --cached`).
///
/// Refuses a renamed file's hunk (see the module docs). Errors — including a
/// patch that no longer fits the index — leave the index untouched.
pub fn stage_hunk(dir: &Path, file: &DiffFile, hunk: &DiffHunk) -> Result<(), Error> {
    let patch = worktree_hunk_patch(dir, file, hunk)?;
    apply(dir, &patch, false)
}

/// Unstage a single `hunk` of `file` from the index (`git apply --cached -R`).
///
/// The patch is taken from the *staged* diff (index vs `HEAD`) so its new side
/// is the index content the reverse-apply expects, which keeps unstaging correct
/// even when the same file has other, still-unstaged hunks.
pub fn unstage_hunk(dir: &Path, file: &DiffFile, hunk: &DiffHunk) -> Result<(), Error> {
    let patch = staged_hunk_patch(dir, file, hunk)?;
    apply(dir, &patch, true)
}

/// Stage every change to `file` (all its hunks) with `git add -A`, which records
/// additions, modifications, and deletions. A renamed file passes both its new
/// and old paths so the whole rename is staged.
pub fn stage_file(dir: &Path, file: &DiffFile) -> Result<(), Error> {
    let mut args = vec!["add", "-A", "--", file.path.as_str()];
    if let Some(old) = &file.old_path {
        args.push(old.as_str());
    }
    git(dir, &args)?;
    Ok(())
}

/// Unstage every change to `file`, restoring its index entry to `HEAD`
/// (`git reset -q HEAD -- <path>`). A renamed file also resets its old path.
pub fn unstage_file(dir: &Path, file: &DiffFile) -> Result<(), Error> {
    let mut args = vec!["reset", "-q", "HEAD", "--", file.path.as_str()];
    if let Some(old) = &file.old_path {
        args.push(old.as_str());
    }
    git(dir, &args)?;
    Ok(())
}

/// The staged changes — the index diffed against `HEAD` (`git diff --cached`) —
/// parsed into the reviewable model. A hunk here is one the reviewer has
/// accepted; the surface renders it as staged.
pub fn staged(dir: &Path) -> Result<Vec<DiffFile>, Error> {
    let out = git(dir, &["diff", "--no-color", "--find-renames", "--cached"])?;
    Ok(diff::parse(&out))
}

/// The unstaged changes — the worktree diffed against the index (`git diff`) —
/// parsed into the reviewable model.
pub fn unstaged(dir: &Path) -> Result<Vec<DiffFile>, Error> {
    let out = git(dir, &["diff", "--no-color", "--find-renames"])?;
    Ok(diff::parse(&out))
}

/// True when the worktree differs from `HEAD` at all (staged or not) — the gate
/// the surface uses to decide whether per-hunk staging is meaningful for a run.
/// A clean (e.g. already committed) worktree returns false.
pub fn has_worktree_changes(dir: &Path) -> Result<bool, Error> {
    Ok(!git_capture(dir, &["diff", "--quiet", "HEAD"])?.success)
}

/// True when the index holds a curated *subset*: something is staged **and**
/// something is still left unstaged in the worktree. This is the trigger for the
/// merge flow to commit only the accepted part ([`commit_staged`]) instead of
/// the whole worktree — if everything or nothing is staged, committing the whole
/// tree is equivalent, so this is deliberately false in those cases.
///
/// Untracked files are not considered "unstaged" here (they are never part of a
/// hunk-staging selection); only tracked worktree modifications are.
pub fn has_staged_subset(dir: &Path) -> Result<bool, Error> {
    let staged_dirty = !git_capture(dir, &["diff", "--cached", "--quiet"])?.success;
    let unstaged_dirty = !git_capture(dir, &["diff", "--quiet"])?.success;
    Ok(staged_dirty && unstaged_dirty)
}

/// Commit exactly what is currently staged in the index — no implicit `git add`,
/// so a partially staged worktree yields a commit of only the accepted hunks and
/// leaves the rest uncommitted. Returns `false` (committing nothing) when the
/// index holds no staged changes. The commit identity is scoped to this call.
pub fn commit_staged(dir: &Path, message: &str) -> Result<bool, Error> {
    if git_capture(dir, &["diff", "--cached", "--quiet"])?.success {
        return Ok(false);
    }
    let mut args = IDENTITY.to_vec();
    args.extend_from_slice(&["commit", "-m", message]);
    git(dir, &args)?;
    Ok(true)
}

/// Build the minimal patch for staging `hunk` forward: the file's header plus
/// that one hunk, sliced verbatim from the worktree-vs-`HEAD` diff (old side is
/// `HEAD`, matching the index for any not-yet-staged region).
fn worktree_hunk_patch(dir: &Path, file: &DiffFile, hunk: &DiffHunk) -> Result<String, Error> {
    refuse_rename(file)?;
    let raw = git(
        dir,
        &["diff", "--no-color", "HEAD", "--", file.path.as_str()],
    )?;
    extract_hunk(&raw, hunk.old_start).ok_or_else(|| {
        Error::Git("the hunk is no longer present in the worktree; refresh the review".to_string())
    })
}

/// Build the minimal patch for unstaging `hunk`: sliced from the staged diff
/// (index vs `HEAD`), whose new side is the index the reverse-apply targets.
fn staged_hunk_patch(dir: &Path, file: &DiffFile, hunk: &DiffHunk) -> Result<String, Error> {
    refuse_rename(file)?;
    let raw = git(
        dir,
        &["diff", "--no-color", "--cached", "--", file.path.as_str()],
    )?;
    extract_hunk(&raw, hunk.old_start)
        .ok_or_else(|| Error::Git("the hunk is not currently staged".to_string()))
}

fn refuse_rename(file: &DiffFile) -> Result<(), Error> {
    if file.status == FileStatus::Renamed {
        return Err(Error::Git(
            "staging individual hunks of a renamed file is not supported; stage the whole file"
                .to_string(),
        ));
    }
    Ok(())
}

/// Slice a single-file `git diff` into a minimal patch carrying its header and
/// only the hunk whose old-side range starts at `old_start`. Operates on raw
/// bytes (splitting on `\n` inclusively) so `\ No newline at end of file`
/// markers and `\r` bytes survive untouched. Returns `None` when no hunk starts
/// there (already staged, or the diff no longer contains it).
fn extract_hunk(raw: &str, old_start: u32) -> Option<String> {
    // Everything before the first "@@" is the file header (`diff --git`, the
    // `index`/mode lines, `---`/`+++`) — kept verbatim. `git diff -- <path>`
    // yields a single file section, so there is exactly one such header.
    let mut header_end = None;
    for (idx, segment) in raw.split_inclusive('\n').enumerate() {
        if segment.starts_with("@@") {
            header_end = Some(idx);
            break;
        }
    }
    let header_end = header_end?;
    let segments: Vec<&str> = raw.split_inclusive('\n').collect();
    let header: String = segments[..header_end].concat();

    let mut i = header_end;
    while i < segments.len() {
        if segments[i].starts_with("@@") {
            let start = i;
            let this_start = hunk_old_start(segments[i]);
            i += 1;
            while i < segments.len()
                && !segments[i].starts_with("@@")
                && !segments[i].starts_with("diff --git ")
            {
                i += 1;
            }
            if this_start == Some(old_start) {
                let body: String = segments[start..i].concat();
                let mut patch = header;
                patch.push_str(&body);
                if !patch.ends_with('\n') {
                    patch.push('\n');
                }
                return Some(patch);
            }
        } else if segments[i].starts_with("diff --git ") {
            break;
        } else {
            i += 1;
        }
    }
    None
}

/// The old-side start line of an `@@ -old,len +new,len @@` header.
fn hunk_old_start(head: &str) -> Option<u32> {
    let ranges = head.strip_prefix("@@ ")?;
    let old = ranges.split_whitespace().next()?.strip_prefix('-')?;
    old.split(',').next()?.parse().ok()
}

/// Feed `patch` to `git apply --cached` (optionally reversed), dry-running
/// `--check` first so a patch that does not fit never partially mutates the
/// index. `--whitespace=nowarn` keeps a trailing-whitespace line from failing an
/// otherwise valid apply.
fn apply(dir: &Path, patch: &str, reverse: bool) -> Result<(), Error> {
    run_apply(dir, patch, reverse, true)?;
    run_apply(dir, patch, reverse, false)
}

fn run_apply(dir: &Path, patch: &str, reverse: bool, check: bool) -> Result<(), Error> {
    let mut args = vec!["apply", "--cached", "--whitespace=nowarn"];
    if reverse {
        args.push("-R");
    }
    if check {
        args.push("--check");
    }
    let out = git_stdin(dir, &args, patch)?;
    if out.success {
        return Ok(());
    }
    let msg = out.stderr.trim();
    Err(Error::Git(if msg.is_empty() {
        "git apply failed".to_string()
    } else {
        msg.to_string()
    }))
}

#[cfg(test)]
#[path = "../tests/stage.rs"]
mod tests;
