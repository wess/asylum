//! Unified-diff capture and parsing into a reviewable model.
//!
//! The signature review surface is the annotatable AI diff: you read what an
//! agent changed, drop inline comments, and feed them back. This module turns
//! `git diff` output into a structured [`DiffFile`]/[`DiffHunk`]/[`DiffLine`]
//! tree the app can render and anchor annotations to.

use std::path::Path;

use crate::run::{git, Error};

/// How a file changed as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

/// A single line within a hunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

/// One line of a diff hunk with its old/new line numbers (where applicable).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    pub content: String,
    /// 1-based line number in the old file (None for added lines).
    pub old_no: Option<u32>,
    /// 1-based line number in the new file (None for removed lines).
    pub new_no: Option<u32>,
}

/// A contiguous change region within a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffHunk {
    /// The `@@ -a,b +c,d @@` header section text (after the ranges).
    pub header: String,
    pub old_start: u32,
    pub new_start: u32,
    pub lines: Vec<DiffLine>,
}

/// All changes to a single file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffFile {
    pub path: String,
    /// For renames, the previous path.
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub hunks: Vec<DiffHunk>,
    /// True when git reported the blob as binary (no textual hunks).
    pub binary: bool,
}

impl DiffFile {
    /// (added, removed) line counts across all hunks.
    pub fn line_stats(&self) -> (usize, usize) {
        let mut added = 0;
        let mut removed = 0;
        for hunk in &self.hunks {
            for line in &hunk.lines {
                match line.kind {
                    LineKind::Added => added += 1,
                    LineKind::Removed => removed += 1,
                    LineKind::Context => {}
                }
            }
        }
        (added, removed)
    }
}

/// Diff the worktree at `dir` against `base` (e.g. `"HEAD"`). Includes staged,
/// unstaged, and — via `--`-less invocation — untracked changes are *not*
/// included (git diff never shows untracked); callers wanting those add them
/// with `git add -N` first.
pub fn against(dir: &Path, base: &str) -> Result<Vec<DiffFile>, Error> {
    let out = git(
        dir,
        &["diff", "--no-color", "--find-renames", base],
    )?;
    Ok(parse(&out))
}

/// Diff the changes on a worktree's branch relative to where it forked from
/// `base_branch` (the merge-base) — the "what has this agent done" view.
pub fn since_fork(dir: &Path, base_branch: &str) -> Result<Vec<DiffFile>, Error> {
    let spec = format!("{base_branch}...HEAD");
    let out = git(dir, &["diff", "--no-color", "--find-renames", &spec])?;
    Ok(parse(&out))
}

/// Parse unified `git diff` output into per-file structures.
pub fn parse(out: &str) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = Vec::new();
    let mut lines = out.lines().peekable();

    while let Some(line) = lines.next() {
        if !line.starts_with("diff --git ") {
            continue;
        }
        let mut file = DiffFile {
            path: parse_diff_git_path(line),
            old_path: None,
            status: FileStatus::Modified,
            hunks: Vec::new(),
            binary: false,
        };

        // Header block up to the first hunk (`@@`) or the next file.
        while let Some(&next) = lines.peek() {
            if next.starts_with("@@") || next.starts_with("diff --git ") {
                break;
            }
            let header = lines.next().unwrap();
            apply_header(&mut file, header);
        }

        // Hunks.
        while let Some(&next) = lines.peek() {
            if next.starts_with("diff --git ") {
                break;
            }
            if next.starts_with("@@") {
                let head = lines.next().unwrap();
                let hunk = parse_hunk(head, &mut lines);
                file.hunks.push(hunk);
            } else {
                lines.next();
            }
        }

        files.push(file);
    }
    files
}

fn parse_diff_git_path(line: &str) -> String {
    // "diff --git a/foo b/foo" — take the b/ side, strip the prefix.
    line.rsplit(" b/")
        .next()
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn apply_header(file: &mut DiffFile, header: &str) {
    if header.starts_with("new file") {
        file.status = FileStatus::Added;
    } else if header.starts_with("deleted file") {
        file.status = FileStatus::Deleted;
    } else if let Some(rest) = header.strip_prefix("rename from ") {
        file.status = FileStatus::Renamed;
        file.old_path = Some(rest.to_string());
    } else if header.starts_with("Binary files") || header.starts_with("GIT binary patch") {
        file.binary = true;
    }
}

fn parse_hunk<'a, I>(head: &str, lines: &mut std::iter::Peekable<I>) -> DiffHunk
where
    I: Iterator<Item = &'a str>,
{
    let (old_start, new_start, header) = parse_hunk_header(head);
    let mut old_no = old_start;
    let mut new_no = new_start;
    let mut out = Vec::new();

    while let Some(&next) = lines.peek() {
        if next.starts_with("@@") || next.starts_with("diff --git ") {
            break;
        }
        let line = lines.next().unwrap();
        let (kind, content) = match line.as_bytes().first() {
            Some(b'+') => (LineKind::Added, &line[1..]),
            Some(b'-') => (LineKind::Removed, &line[1..]),
            Some(b'\\') => continue, // "\ No newline at end of file"
            Some(b' ') => (LineKind::Context, &line[1..]),
            _ => (LineKind::Context, line),
        };
        let (o, n) = match kind {
            LineKind::Added => {
                let n = new_no;
                new_no += 1;
                (None, Some(n))
            }
            LineKind::Removed => {
                let o = old_no;
                old_no += 1;
                (Some(o), None)
            }
            LineKind::Context => {
                let o = old_no;
                let n = new_no;
                old_no += 1;
                new_no += 1;
                (Some(o), Some(n))
            }
        };
        out.push(DiffLine {
            kind,
            content: content.to_string(),
            old_no: o,
            new_no: n,
        });
    }

    DiffHunk {
        header,
        old_start,
        new_start,
        lines: out,
    }
}

/// Parse `@@ -old_start,old_len +new_start,new_len @@ trailing` → starts + text.
fn parse_hunk_header(head: &str) -> (u32, u32, String) {
    let mut old_start = 0;
    let mut new_start = 0;
    let mut header = String::new();
    // Between the second "@@" the trailing section text lives.
    if let Some(idx) = head[2..].find("@@") {
        header = head[2 + idx + 2..].trim().to_string();
        let ranges = &head[2..2 + idx];
        for tok in ranges.split_whitespace() {
            if let Some(rest) = tok.strip_prefix('-') {
                old_start = rest.split(',').next().and_then(|s| s.parse().ok()).unwrap_or(0);
            } else if let Some(rest) = tok.strip_prefix('+') {
                new_start = rest.split(',').next().and_then(|s| s.parse().ok()).unwrap_or(0);
            }
        }
    }
    (old_start, new_start, header)
}

#[cfg(test)]
#[path = "../tests/diff.rs"]
mod tests;
