//! Run a worktree's checks — type-check, lint, test — and report PASS/FAIL.
//!
//! A run's health is shown with PASS/FAIL indicators from type checking,
//! ESLint, and tests. This crate detects the appropriate checks for a project
//! (by the files present), runs them, and classifies each by exit status. The
//! detection and classification are pure and tested; running shells out.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

/// A named check: a command to run in the worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// Short id, e.g. `typecheck`, `lint`, `test`.
    pub id: String,
    /// Human label.
    pub label: String,
    /// Program to run.
    pub program: String,
    /// Arguments.
    pub args: Vec<String>,
}

impl Check {
    fn new(id: &str, label: &str, program: &str, args: &[&str]) -> Self {
        Check {
            id: id.to_string(),
            label: label.to_string(),
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// The outcome of running a [`Check`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    /// The tool wasn't installed / couldn't be launched.
    Skipped,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Skipped => "skipped",
        }
    }
}

/// The result of running a check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub id: String,
    pub status: Status,
    /// Last meaningful line of output, for a compact summary.
    pub summary: String,
    pub duration_ms: u128,
}

/// Detect the checks appropriate for the project rooted at `dir`, from the
/// files it contains. Node projects get typecheck/lint/test; Rust projects get
/// check/clippy/test; both may apply in a polyglot repo.
pub fn detect(dir: &Path) -> Vec<Check> {
    let mut checks = Vec::new();
    let has = |name: &str| dir.join(name).exists();

    if has("package.json") {
        if has("tsconfig.json") {
            checks.push(Check::new(
                "typecheck",
                "Type check",
                "npx",
                &["tsc", "--noEmit"],
            ));
        }
        checks.push(Check::new("lint", "ESLint", "npx", &["eslint", "."]));
        checks.push(Check::new("test", "Tests", "npm", &["test", "--silent"]));
    }
    if has("Cargo.toml") {
        checks.push(Check::new("check", "cargo check", "cargo", &["check"]));
        checks.push(Check::new(
            "clippy",
            "Clippy",
            "cargo",
            &["clippy", "--all-targets"],
        ));
        checks.push(Check::new("test", "cargo test", "cargo", &["test"]));
    }
    checks
}

/// Run a single check in `dir`.
pub fn run(dir: &Path, check: &Check) -> CheckResult {
    let start = Instant::now();
    let output = Command::new(&check.program)
        .args(&check.args)
        .current_dir(dir)
        .output();
    let duration_ms = start.elapsed().as_millis();

    match output {
        Ok(out) => {
            let status = if out.status.success() {
                Status::Pass
            } else {
                Status::Fail
            };
            let summary = last_meaningful_line(&out.stdout, &out.stderr);
            CheckResult {
                id: check.id.clone(),
                status,
                summary,
                duration_ms,
            }
        }
        Err(e) => CheckResult {
            id: check.id.clone(),
            status: Status::Skipped,
            summary: format!("{} not available: {e}", check.program),
            duration_ms,
        },
    }
}

/// Run every check in order, stopping at nothing (each is independent).
pub fn run_all(dir: &Path, checks: &[Check]) -> Vec<CheckResult> {
    checks.iter().map(|c| run(dir, c)).collect()
}

/// Overall status of a set of results: `Fail` if any failed, `Pass` if any
/// passed and none failed, else `Skipped`.
pub fn overall(results: &[CheckResult]) -> Status {
    if results.iter().any(|r| r.status == Status::Fail) {
        Status::Fail
    } else if results.iter().any(|r| r.status == Status::Pass) {
        Status::Pass
    } else {
        Status::Skipped
    }
}

/// The last non-empty line across stderr (preferred) then stdout — the bit most
/// tools put their summary on.
fn last_meaningful_line(stdout: &[u8], stderr: &[u8]) -> String {
    let pick = |bytes: &[u8]| -> Option<String> {
        String::from_utf8_lossy(bytes)
            .lines()
            .rev()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .map(str::to_string)
    };
    pick(stderr).or_else(|| pick(stdout)).unwrap_or_default()
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
