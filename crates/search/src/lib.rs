//! Cross-worktree content search.
//!
//! Keyboard-native search finds code across every worktree without an app
//! switch. This crate drives `ripgrep` (fast, respects `.gitignore`) and falls
//! back to `git grep` when `rg` isn't installed, parsing the shared
//! `file:line:col:text` "vimgrep" format into typed [`Match`]es. The parser is
//! pure and tested; the search runs a subprocess.

use std::path::Path;
use std::process::Command;

/// A search failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no search backend: install ripgrep (rg) or run inside a git repo")]
    NoBackend,
    #[error("search failed: {0}")]
    Failed(String),
}

/// A single match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub text: String,
}

/// Search options.
#[derive(Debug, Clone)]
pub struct Options {
    /// Case-insensitive matching.
    pub ignore_case: bool,
    /// Treat the pattern as a fixed string, not a regex.
    pub fixed: bool,
    /// Cap the number of results (0 = unlimited).
    pub max_results: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            ignore_case: true,
            fixed: false,
            max_results: 500,
        }
    }
}

/// Search `dir` for `pattern`, preferring ripgrep. Returns matches in file
/// order. An empty result is `Ok(vec![])`, not an error.
pub fn search(dir: &Path, pattern: &str, opts: &Options) -> Result<Vec<Match>, Error> {
    if let Some(out) = run_rg(dir, pattern, opts) {
        return Ok(cap(parse_vimgrep(&out), opts.max_results));
    }
    if let Some(out) = run_git_grep(dir, pattern, opts) {
        return Ok(cap(parse_vimgrep(&out), opts.max_results));
    }
    Err(Error::NoBackend)
}

fn cap(mut matches: Vec<Match>, max: usize) -> Vec<Match> {
    if max > 0 && matches.len() > max {
        matches.truncate(max);
    }
    matches
}

fn run_rg(dir: &Path, pattern: &str, opts: &Options) -> Option<String> {
    let mut args = vec!["--vimgrep", "--no-heading", "--color=never"];
    if opts.ignore_case {
        args.push("-i");
    }
    if opts.fixed {
        args.push("-F");
    }
    args.push(pattern);
    let out = Command::new("rg")
        .args(&args)
        .current_dir(dir)
        .output()
        .ok()?;
    // rg exits 2 on a real error; 0/1 are match/no-match (both fine).
    if out.status.code() == Some(2) {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn run_git_grep(dir: &Path, pattern: &str, opts: &Options) -> Option<String> {
    // git grep -n --column emits `file:line:col:text`.
    let mut args = vec!["grep", "-n", "--column", "--no-color"];
    if opts.ignore_case {
        args.push("-i");
    }
    if opts.fixed {
        args.push("-F");
    }
    args.push("-e");
    args.push(pattern);
    let out = Command::new("git")
        .args(&args)
        .current_dir(dir)
        .output()
        .ok()?;
    // git grep exits 1 when nothing matches; >1 is a real error.
    if out.status.code().unwrap_or(1) > 1 {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Parse `file:line:col:text` lines (ripgrep `--vimgrep` / `git grep --column`).
pub fn parse_vimgrep(out: &str) -> Vec<Match> {
    out.lines()
        .filter_map(|line| {
            // Split into at most 4 parts; the text may itself contain colons.
            let mut it = line.splitn(4, ':');
            let file = it.next()?;
            let line_no = it.next()?.parse().ok()?;
            let column = it.next()?.parse().ok()?;
            let text = it.next()?;
            if file.is_empty() {
                return None;
            }
            Some(Match {
                file: file.to_string(),
                line: line_no,
                column,
                text: text.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
