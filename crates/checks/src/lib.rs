//! Run a worktree's checks - type-check, lint, test - and report PASS/FAIL.
//!
//! A run's health is shown with PASS/FAIL indicators derived from the project's
//! ecosystem: JavaScript/TypeScript package scripts (run through the detected
//! package manager - bun, npm, pnpm, or yarn), Cargo commands, Python (ruff +
//! pytest, venv-aware), and Go (build/vet/test). This crate detects the
//! appropriate checks for a project, runs them under a deadline, and classifies
//! each by exit status. Detection and classification are pure and tested;
//! running shells out.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
        checks.push(python_check(
            dir,
            "python/lint",
            "Ruff",
            "ruff",
            &["check", "."],
        ));
        checks.push(python_check(
            dir,
            "python/test",
            "pytest",
            "pytest",
            &["-q"],
        ));
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

/// The venv's script directory: `bin` on unix, `Scripts` on Windows.
#[cfg(windows)]
const VENV_BIN: &str = "Scripts";
#[cfg(not(windows))]
const VENV_BIN: &str = "bin";

#[cfg(windows)]
fn venv_exe_name(tool: &str) -> String {
    format!("{tool}.exe")
}
#[cfg(not(windows))]
fn venv_exe_name(tool: &str) -> String {
    tool.to_string()
}

/// The venv's own interpreter, if it has one.
#[cfg(windows)]
fn venv_python(venv: &Path) -> Option<PathBuf> {
    let python = venv.join("Scripts").join("python.exe");
    python.is_file().then_some(python)
}
#[cfg(not(windows))]
fn venv_python(venv: &Path) -> Option<PathBuf> {
    ["python", "python3"].into_iter().find_map(|name| {
        let python = venv.join("bin").join(name);
        python.is_file().then_some(python)
    })
}

/// Whether `tool` looks importable from the venv's interpreter: a `<tool>`
/// entry under its `site-packages`. A filesystem heuristic, not a real
/// import - good enough to prefer `python -m <tool>` over a PATH lookup that
/// would otherwise miss a venv-only install entirely.
#[cfg(windows)]
fn venv_has_module(venv: &Path, tool: &str) -> bool {
    venv.join("Lib").join("site-packages").join(tool).exists()
}
#[cfg(not(windows))]
fn venv_has_module(venv: &Path, tool: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(venv.join("lib")) else {
        return false;
    };
    entries
        .flatten()
        .any(|entry| entry.path().join("site-packages").join(tool).exists())
}

/// Resolve how to invoke a Python `tool` from the project at `dir`, preferring
/// a project virtualenv over whatever (if anything) is on `PATH`: the venv's
/// own console script, then the venv's interpreter as `python -m <tool>` once
/// the module is confirmed present, then the bare tool name for [`run`] to
/// look up on `PATH` as before.
fn resolve_python_tool(dir: &Path, tool: &str, extra_args: &[&str]) -> (String, Vec<String>) {
    let to_args = |args: &[&str]| args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let venv = dir.join(".venv");
    if venv.is_dir() {
        let script = venv.join(VENV_BIN).join(venv_exe_name(tool));
        if script.is_file() {
            return (script.to_string_lossy().into_owned(), to_args(extra_args));
        }
        if venv_has_module(&venv, tool) {
            if let Some(python) = venv_python(&venv) {
                let mut args = vec!["-m".to_string(), tool.to_string()];
                args.extend(extra_args.iter().map(|s| s.to_string()));
                return (python.to_string_lossy().into_owned(), args);
            }
        }
    }
    (tool.to_string(), to_args(extra_args))
}

/// Build a Python-ecosystem [`Check`], resolving `tool` through
/// [`resolve_python_tool`].
fn python_check(dir: &Path, id: &str, label: &str, tool: &str, extra_args: &[&str]) -> Check {
    let (program, args) = resolve_python_tool(dir, tool, extra_args);
    Check {
        id: id.to_string(),
        label: label.to_string(),
        program,
        args,
    }
}

/// Default per-command deadline: generous enough for a slow test suite, short
/// enough that a hung process - one waiting on input, one stuck retrying -
/// doesn't block verification forever.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);

/// How often [`run_with_timeout`] polls the child for exit while waiting.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Run a single check in `dir`, under [`DEFAULT_TIMEOUT`]. See
/// [`run_with_timeout`] to configure the deadline.
pub fn run(dir: &Path, check: &Check) -> CheckResult {
    run_with_timeout(dir, check, DEFAULT_TIMEOUT)
}

/// Run a single check in `dir`, killing it and reporting `Fail` if it is
/// still running after `timeout` - a hung or input-waiting check is a red
/// flag, not something to silently skip. Stdin is closed so the check can
/// never block waiting on input; stdout/stderr are drained on background
/// threads so a chatty check can't deadlock on a full pipe while nothing is
/// reading it.
pub fn run_with_timeout(dir: &Path, check: &Check, timeout: Duration) -> CheckResult {
    let start = Instant::now();
    let mut command = Command::new(&check.program);
    command
        .args(&check.args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // A fresh process group (pgid == the child's pid) so a timeout can
        // signal the whole tree, not just the direct child.
        command.process_group(0);
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            return CheckResult {
                id: check.id.clone(),
                status: Status::Skipped,
                summary: format!("{} not available: {e}", check.program),
                duration_ms: start.elapsed().as_millis(),
            };
        }
    };

    let stdout = spawn_reader(child.stdout.take());
    let stderr = spawn_reader(child.stderr.take());
    let outcome = wait_with_timeout(&mut child, timeout);
    if matches!(outcome, Outcome::TimedOut) {
        kill_tree(&mut child);
        let _ = child.wait();
    }
    let duration_ms = start.elapsed().as_millis();
    let out = stdout.join().unwrap_or_default();
    let err = stderr.join().unwrap_or_default();

    match outcome {
        Outcome::Exited(status) => CheckResult {
            id: check.id.clone(),
            status: if status.success() {
                Status::Pass
            } else {
                Status::Fail
            },
            summary: last_meaningful_line(&out, &err),
            duration_ms,
        },
        Outcome::TimedOut => CheckResult {
            id: check.id.clone(),
            status: Status::Fail,
            summary: format!("timed out after {}s", timeout.as_secs()),
            duration_ms,
        },
        Outcome::WaitError(e) => CheckResult {
            id: check.id.clone(),
            status: Status::Fail,
            summary: format!("wait failed: {e}"),
            duration_ms,
        },
    }
}

/// Run every check in order under [`DEFAULT_TIMEOUT`], stopping at nothing
/// (each is independent). See [`run_all_with_timeout`] to configure the
/// deadline.
pub fn run_all(dir: &Path, checks: &[Check]) -> Vec<CheckResult> {
    run_all_with_timeout(dir, checks, DEFAULT_TIMEOUT)
}

/// Run every check in order, each under its own `timeout`.
pub fn run_all_with_timeout(dir: &Path, checks: &[Check], timeout: Duration) -> Vec<CheckResult> {
    checks
        .iter()
        .map(|c| run_with_timeout(dir, c, timeout))
        .collect()
}

/// The outcome of waiting for a child within a deadline.
enum Outcome {
    Exited(ExitStatus),
    TimedOut,
    WaitError(std::io::Error),
}

/// Poll `child` for exit until it finishes or `timeout` elapses.
fn wait_with_timeout(child: &mut Child, timeout: Duration) -> Outcome {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Outcome::Exited(status),
            Ok(None) => {
                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    return Outcome::TimedOut;
                }
                thread::sleep(POLL_INTERVAL.min(timeout - elapsed));
            }
            Err(e) => return Outcome::WaitError(e),
        }
    }
}

/// Drain a pipe to completion on a background thread, so a chatty child can't
/// fill the pipe buffer and block on a write while nothing is reading it.
fn spawn_reader<R: Read + Send + 'static>(pipe: Option<R>) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    })
}

/// Kill a timed-out check's whole process tree where that's straightforward:
/// on unix, `process_group(0)` at spawn time made the child's pid double as
/// its process group id, so signalling `-pid` reaches it and any descendants
/// that kept the inherited group. Elsewhere, just the direct child.
#[cfg(unix)]
fn kill_tree(child: &mut Child) {
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
}
#[cfg(not(unix))]
fn kill_tree(child: &mut Child) {
    let _ = child.kill();
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
