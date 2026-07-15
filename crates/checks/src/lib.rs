//! Run a worktree's checks - type-check, lint, test - and report PASS/FAIL.
//!
//! A run's health is shown with PASS/FAIL indicators derived from the project's
//! ecosystem: JavaScript/TypeScript package scripts (run through the detected
//! package manager - bun, npm, pnpm, or yarn), Cargo commands, Python (ruff +
//! pytest), and Go (build/vet/test). This crate detects the appropriate checks
//! for a project, runs them, and classifies each by exit status. Detection and
//! classification are pure and tested; running shells out.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use serde_json::Value;

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
/// files it contains. JavaScript projects run only scripts the package declares
/// (through the detected package manager); Rust, Python, and Go projects get
/// their standard trio. Any combination may apply in a polyglot repo.
pub fn detect(dir: &Path) -> Vec<Check> {
    let mut checks = Vec::new();
    let has = |name: &str| dir.join(name).exists();

    if has("package.json") {
        let manager = package_manager(dir);
        let scripts = std::fs::read_to_string(dir.join("package.json"))
            .ok()
            .and_then(|source| serde_json::from_str::<Value>(&source).ok())
            .and_then(|package| package.get("scripts")?.as_object().cloned())
            .unwrap_or_default();
        for (id, label) in [
            ("typecheck", "Type check"),
            ("lint", "Lint"),
            ("test", "Tests"),
        ] {
            if scripts.get(id).is_some_and(Value::is_string) {
                checks.push(Check::new(
                    &format!("{manager}/{id}"),
                    label,
                    manager,
                    &["run", id],
                ));
            }
        }
    }
    if has("Cargo.toml") {
        checks.push(Check::new(
            "cargo/check",
            "cargo check",
            "cargo",
            &["check"],
        ));
        checks.push(Check::new(
            "cargo/clippy",
            "Clippy",
            "cargo",
            &["clippy", "--all-targets"],
        ));
        checks.push(Check::new("cargo/test", "cargo test", "cargo", &["test"]));
    }
    if has("pyproject.toml") || has("requirements.txt") || has("setup.py") {
        checks.push(Check::new("python/lint", "Ruff", "ruff", &["check", "."]));
        checks.push(Check::new("python/test", "pytest", "pytest", &["-q"]));
    }
    if has("go.mod") {
        checks.push(Check::new(
            "go/build",
            "go build",
            "go",
            &["build", "./..."],
        ));
        checks.push(Check::new("go/vet", "go vet", "go", &["vet", "./..."]));
        checks.push(Check::new("go/test", "go test", "go", &["test", "./..."]));
    }
    checks
}

/// Pick the JavaScript package manager to run scripts with, from the lockfile
/// present in `dir`. Defaults to bun when no lockfile disambiguates.
fn package_manager(dir: &Path) -> &'static str {
    let has = |name: &str| dir.join(name).exists();
    if has("pnpm-lock.yaml") {
        "pnpm"
    } else if has("yarn.lock") {
        "yarn"
    } else if has("package-lock.json") {
        "npm"
    } else {
        "bun"
    }
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

/// The last non-empty line across stderr (preferred) then stdout - the bit most
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
