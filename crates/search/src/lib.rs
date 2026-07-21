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
    #[error("invalid search pattern: {0}")]
    InvalidPattern(String),
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
/// order. An empty result is `Ok(vec![])`, not an error. Returns `InvalidPattern`
/// if the pattern is malformed; returns `NoBackend` if neither rg nor git grep
/// is available.
pub fn search(dir: &Path, pattern: &str, opts: &Options) -> Result<Vec<Match>, Error> {
    match try_rg(dir, pattern, opts) {
        Ok(Some(out)) => return Ok(cap(parse_vimgrep(&out), opts.max_results)),
        Err(e) => return Err(e),
        Ok(None) => {}
    }
    match try_git_grep(dir, pattern, opts) {
        Ok(Some(out)) => return Ok(cap(parse_vimgrep(&out), opts.max_results)),
        Err(e) => return Err(e),
        Ok(None) => {}
    }
    Err(Error::NoBackend)
}

fn cap(mut matches: Vec<Match>, max: usize) -> Vec<Match> {
    if max > 0 && matches.len() > max {
        matches.truncate(max);
    }
    matches
}

fn try_rg(dir: &Path, pattern: &str, opts: &Options) -> Result<Option<String>, Error> {
    let mut args = vec!["--vimgrep", "--no-heading", "--color=never"];
    if opts.ignore_case {
        args.push("-i");
    }
    if opts.fixed {
        args.push("-F");
    }
    args.push(pattern);
    let out = match Command::new("rg").args(&args).current_dir(dir).output() {
        Ok(o) => o,
        Err(_) => return Ok(None), // rg not installed
    };
    // rg exits 2 on a real error; 0/1 are match/no-match (both fine).
    if out.status.code() == Some(2) {
        let stderr = String::from_utf8_lossy(&out.stderr);
        // Extract the first line of the error message, trimming whitespace.
        if let Some(line) = stderr.lines().next() {
            let msg = line.trim().to_string();
            return Err(Error::InvalidPattern(msg));
        }
        return Ok(None); // Couldn't parse stderr, treat as tool missing.
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()))
}

fn try_git_grep(dir: &Path, pattern: &str, opts: &Options) -> Result<Option<String>, Error> {
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
    let out = match Command::new("git").args(&args).current_dir(dir).output() {
        Ok(o) => o,
        Err(_) => return Ok(None), // git not available or not a repo
    };
    // git grep exits 1 when nothing matches; >1 is a real error.
    if out.status.code().unwrap_or(1) > 1 {
        let stderr = String::from_utf8_lossy(&out.stderr);
        // Extract the first line of the error message, trimming whitespace.
        if let Some(line) = stderr.lines().next() {
            let msg = line.trim().to_string();
            return Err(Error::InvalidPattern(msg));
        }
        return Ok(None); // Couldn't parse stderr, treat as tool missing.
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()))
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
