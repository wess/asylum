//! Working-tree status via `git status --porcelain=v2`.
//!
//! The ADE shows a live per-worktree change summary (how many files an agent
//! touched, staged vs unstaged, untracked). Porcelain v2 is stable and
//! machine-parseable across git versions.

use std::path::Path;

use crate::run::{git, Error};

/// The kind of change to a path, collapsed from porcelain's XY status codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Untracked,
    Ignored,
    Conflicted,
}

/// One changed path in the working tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub kind: StatusKind,
    /// True when the change (or part of it) is staged in the index.
    pub staged: bool,
}

/// Collect the changed paths in the worktree rooted at (or under) `dir`.
pub fn status(dir: &Path) -> Result<Vec<Entry>, Error> {
    let out = git(dir, &["status", "--porcelain=v2", "--untracked-files=all"])?;
    Ok(parse(&out))
}

/// A compact (added, modified, deleted) count over a status listing.
pub fn summarize(entries: &[Entry]) -> (usize, usize, usize) {
    let mut added = 0;
    let mut modified = 0;
    let mut deleted = 0;
    for e in entries {
        match e.kind {
            StatusKind::Added | StatusKind::Untracked => added += 1,
            StatusKind::Deleted => deleted += 1,
            _ => modified += 1,
        }
    }
    (added, modified, deleted)
}

/// Remove entries owned by a managed directory such as the app's nested
/// worktree root. Absolute managed paths cannot appear in repository status.
pub fn excluding_prefix(entries: Vec<Entry>, prefix: &Path) -> Vec<Entry> {
    if prefix.is_absolute() {
        return entries;
    }
    entries
        .into_iter()
        .filter(|entry| !Path::new(&entry.path).starts_with(prefix))
        .collect()
}

fn parse(out: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    for line in out.lines() {
        let mut parts = line.splitn(2, ' ');
        let tag = parts.next().unwrap_or("");
        let rest = parts.next().unwrap_or("");
        match tag {
            // "1" ordinary change, "2" rename/copy: XY <sub> ... <path>
            "1" | "2" => {
                if let Some(entry) = parse_changed(tag, rest) {
                    entries.push(entry);
                }
            }
            // "?" untracked, "!" ignored: <path>
            "?" => entries.push(Entry {
                path: rest.to_string(),
                kind: StatusKind::Untracked,
                staged: false,
            }),
            "!" => entries.push(Entry {
                path: rest.to_string(),
                kind: StatusKind::Ignored,
                staged: false,
            }),
            // "u" unmerged (conflict): XY sub m1 m2 m3 mW h1 h2 h3 <path> - 9
            // fixed fields before the path.
            "u" => {
                if let Some(path) = fixed_fields_tail(rest, 9) {
                    entries.push(Entry {
                        path: path.to_string(),
                        kind: StatusKind::Conflicted,
                        staged: false,
                    });
                }
            }
            _ => {}
        }
    }
    entries
}

/// Parse a changed-entry line ("1"/"2"). The XY field is the two-char status;
/// X is the index (staged) state, Y is the working-tree state. Whatever
/// follows the fixed-width fields is the path verbatim - it may itself
/// contain spaces, so it must not be recovered with a whitespace split. For a
/// rename ("2") that remainder is `<path>\t<origPath>`; we keep the new path.
fn parse_changed(tag: &str, rest: &str) -> Option<Entry> {
    let mut fields = rest.split(' ');
    let xy = fields.next()?;
    let x = xy.as_bytes().first().copied()?;
    let y = xy.as_bytes().get(1).copied()?;
    let staged = x != b'.';
    let code = if x != b'.' { x } else { y };
    let kind = match code {
        b'A' => StatusKind::Added,
        b'D' => StatusKind::Deleted,
        b'R' | b'C' => StatusKind::Renamed,
        _ if tag == "2" => StatusKind::Renamed,
        _ => StatusKind::Modified,
    };
    // "1" (ordinary) has 7 fixed fields before <path>: XY sub mH mI mW hH hI.
    // "2" (rename/copy) has an eighth, the <X><score> field, before <path>.
    let fixed = if tag == "2" { 8 } else { 7 };
    let tail = fixed_fields_tail(rest, fixed)?;
    let path = tail.split('\t').next().unwrap_or(tail).to_string();
    Some(Entry { path, kind, staged })
}

/// Drop `n` leading space-separated fixed-width fields from `s` and return
/// whatever follows verbatim. Porcelain v2 pads every record with a known
/// number of fixed fields before the trailing path, so this recovers a path
/// that itself contains spaces instead of truncating it the way splitting on
/// the *last* space would.
fn fixed_fields_tail(s: &str, n: usize) -> Option<&str> {
    let mut rest = s;
    for _ in 0..n {
        rest = rest.split_once(' ')?.1;
    }
    Some(rest)
}

#[cfg(test)]
#[path = "../tests/status.rs"]
mod tests;
