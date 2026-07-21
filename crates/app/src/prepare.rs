//! Project setup commands run in a freshly-created worktree.
//!
//! A project's `asylum.toml` may list `setup` commands (install deps, build,
//! …) run once when a worktree is created, before its agent starts. Each runs
//! as its own child with stdout/stderr captured separately and its command
//! line, duration, and exit status recorded — a per-command transcript, not one
//! concatenated blob. Commands run sequentially and stop at the first failure
//! (later commands often depend on earlier ones). Every command is bounded by a
//! generous deadline so a hung install can't wedge preparation forever, and the
//! whole sequence is cancellable: a flipped flag kills the running command's
//! process group (mirroring the checks/reap teardown) and stops the rest.
//!
//! Running shells out; the formatting and outcome logic are pure and tested.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Generous per-command deadline: a slow `bun install` / `cargo build` finishes
/// comfortably, but a command hung on input or a stuck retry can't block
/// preparation forever. Mirrors the checks crate's deadline.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);

/// How often the wait loop polls the child (and the cancel flag) while running.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// How many trailing output lines a failure message carries.
const TAIL_LINES: usize = 12;

/// How one setup command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// Exited zero.
    Ok,
    /// Exited non-zero (carries the code).
    Failed(i32),
    /// Could not be launched at all (shell missing, etc.).
    Unstartable,
    /// Killed after exceeding the deadline.
    TimedOut,
    /// Killed, or never started, because preparation was cancelled.
    Cancelled,
}

impl Disposition {
    fn is_ok(self) -> bool {
        matches!(self, Disposition::Ok)
    }
}

/// One setup command's captured result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    /// The shell command line, verbatim from `asylum.toml`.
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub disposition: Disposition,
    pub duration_ms: u128,
}

impl CommandResult {
    /// A synthetic entry for a command that was never started because the
    /// sequence was cancelled before reaching it — records *where* it stopped.
    fn cancelled_before(command: &str) -> Self {
        CommandResult {
            command: command.to_string(),
            stdout: String::new(),
            stderr: String::new(),
            disposition: Disposition::Cancelled,
            duration_ms: 0,
        }
    }

    /// The stream a tool's summary usually lands on: stderr when it has
    /// content, otherwise stdout.
    fn meaningful(&self) -> &str {
        if self.stderr.trim().is_empty() {
            &self.stdout
        } else {
            &self.stderr
        }
    }

    fn duration_secs(&self) -> String {
        format!("{:.1}s", self.duration_ms as f64 / 1000.0)
    }
}

/// The report-level disposition of a whole setup sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupOutcome {
    /// Every command exited zero (or there were none).
    Ok,
    /// A command failed, timed out, or could not start.
    Failed,
    /// Preparation was cancelled part-way through.
    Cancelled,
}

/// The result of running a worktree's whole setup sequence, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetupReport {
    pub results: Vec<CommandResult>,
}

impl SetupReport {
    /// The sequence's outcome, derived from where it stopped. Because the loop
    /// stops at the first non-`Ok` command, the last result tells the story.
    pub fn outcome(&self) -> SetupOutcome {
        match self.results.last().map(|r| r.disposition) {
            None | Some(Disposition::Ok) => SetupOutcome::Ok,
            Some(Disposition::Cancelled) => SetupOutcome::Cancelled,
            Some(_) => SetupOutcome::Failed,
        }
    }

    /// The command that ended the sequence, when it ended unhappily.
    fn terminal_command(&self) -> Option<&CommandResult> {
        self.results.last().filter(|r| !r.disposition.is_ok())
    }
}

/// Run every setup command from `config` in `worktree`, in order, stopping at
/// the first failure. `cancel` is polled while a command runs and between
/// commands; when it flips, the running command's process group is killed and
/// no further commands start. Each command is bounded by `timeout`.
pub fn run(
    worktree: &Path,
    config: &config::ProjectConfig,
    cancel: &Arc<AtomicBool>,
    timeout: Duration,
) -> SetupReport {
    let mut results = Vec::new();
    for command in &config.setup {
        if cancel.load(Ordering::Relaxed) {
            results.push(CommandResult::cancelled_before(command));
            break;
        }
        let result = run_one(worktree, command, &config.env, cancel, timeout);
        let stop = !result.disposition.is_ok();
        results.push(result);
        if stop {
            break;
        }
    }
    SetupReport { results }
}

/// Build the shell invocation for one command, matching the login-shell form
/// setup has always used (so `PATH` from the user's profile is present).
fn shell(command: &str) -> Command {
    #[cfg(unix)]
    {
        let mut cmd = Command::new("sh");
        cmd.arg("-lc").arg(command);
        cmd
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    }
}

/// Run a single command, capturing stdout/stderr separately, under `timeout`
/// and the shared `cancel` flag. Stdin is closed so a command can't block
/// waiting on input; stdout/stderr are drained on background threads so a
/// chatty command can't deadlock on a full pipe.
fn run_one(
    worktree: &Path,
    command: &str,
    env: &BTreeMap<String, String>,
    cancel: &Arc<AtomicBool>,
    timeout: Duration,
) -> CommandResult {
    let start = Instant::now();
    let mut cmd = shell(command);
    cmd.current_dir(worktree)
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // A fresh process group (pgid == the child's pid) so a cancel or a
        // timeout can signal the whole tree, not just the direct child.
        cmd.process_group(0);
    }

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            return CommandResult {
                command: command.to_string(),
                stdout: String::new(),
                stderr: format!("could not run `{command}`: {e}"),
                disposition: Disposition::Unstartable,
                duration_ms: start.elapsed().as_millis(),
            };
        }
    };

    let stdout = spawn_reader(child.stdout.take());
    let stderr = spawn_reader(child.stderr.take());
    let wait = wait_for(&mut child, cancel, timeout);
    if matches!(wait, Wait::TimedOut | Wait::Cancelled) {
        kill_tree(&mut child);
        let _ = child.wait();
    }
    let duration_ms = start.elapsed().as_millis();
    let out = String::from_utf8_lossy(&stdout.join().unwrap_or_default()).into_owned();
    let mut err = String::from_utf8_lossy(&stderr.join().unwrap_or_default()).into_owned();

    let disposition = match wait {
        Wait::Exited(status) if status.success() => Disposition::Ok,
        Wait::Exited(status) => Disposition::Failed(status.code().unwrap_or(-1)),
        Wait::TimedOut => Disposition::TimedOut,
        Wait::Cancelled => Disposition::Cancelled,
        Wait::Errored(e) => {
            if !err.is_empty() {
                err.push('\n');
            }
            err.push_str(&format!("wait failed: {e}"));
            Disposition::Failed(-1)
        }
    };
    CommandResult {
        command: command.to_string(),
        stdout: out,
        stderr: err,
        disposition,
        duration_ms,
    }
}

/// The outcome of waiting for a child within its deadline and cancel flag.
enum Wait {
    Exited(ExitStatus),
    TimedOut,
    Cancelled,
    Errored(std::io::Error),
}

/// Poll `child` for exit until it finishes, `cancel` flips, or `timeout`
/// elapses.
fn wait_for(child: &mut Child, cancel: &Arc<AtomicBool>, timeout: Duration) -> Wait {
    let start = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Wait::Cancelled;
        }
        match child.try_wait() {
            Ok(Some(status)) => return Wait::Exited(status),
            Ok(None) => {
                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    return Wait::TimedOut;
                }
                thread::sleep(POLL_INTERVAL.min(timeout - elapsed));
            }
            Err(e) => return Wait::Errored(e),
        }
    }
}

/// Drain a pipe to completion on a background thread.
fn spawn_reader<R: Read + Send + 'static>(pipe: Option<R>) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    })
}

/// Kill a command's whole process tree where that's straightforward: on unix
/// `process_group(0)` made the child's pid double as its group id, so
/// signalling `-pid` reaches it and its descendants. Elsewhere, the direct
/// child.
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

/// A plain-language error for a failed setup: names the command, its exit
/// status, and a tail of its own output. `None` for an ok or cancelled report
/// (a cancel is not a failure).
pub fn failure_message(report: &SetupReport) -> Option<String> {
    let failed = report.terminal_command()?;
    if matches!(failed.disposition, Disposition::Cancelled) {
        return None;
    }
    let headline = headline(failed);
    let tail = tail(failed.meaningful(), TAIL_LINES);
    Some(if tail.trim().is_empty() {
        headline
    } else {
        format!("{headline}\n\n{tail}")
    })
}

/// The one-line headline for a command's disposition.
fn headline(result: &CommandResult) -> String {
    match result.disposition {
        Disposition::Failed(code) => {
            format!("Setup command failed: {} (exit {code})", result.command)
        }
        Disposition::TimedOut => format!("Setup command timed out: {}", result.command),
        Disposition::Unstartable => {
            format!("Setup command could not start: {}", result.command)
        }
        Disposition::Cancelled => format!("Setup cancelled during: {}", result.command),
        Disposition::Ok => format!("Setup command succeeded: {}", result.command),
    }
}

/// The full per-command transcript for the run's stored output: each command
/// with a status header and its captured output, in order.
pub fn transcript(report: &SetupReport) -> String {
    report
        .results
        .iter()
        .map(section)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn section(result: &CommandResult) -> String {
    let status = match result.disposition {
        Disposition::Ok => "ok".to_string(),
        Disposition::Failed(code) => format!("exit {code}"),
        Disposition::TimedOut => "timed out".to_string(),
        Disposition::Unstartable => "could not start".to_string(),
        Disposition::Cancelled => "cancelled".to_string(),
    };
    let mut section = format!(
        "$ {}\n[{status}, {}]",
        result.command,
        result.duration_secs()
    );
    let body = combined(result);
    if !body.trim().is_empty() {
        section.push('\n');
        section.push_str(body.trim_end());
    }
    section
}

/// stdout and stderr joined for the transcript, dropping an empty stream.
fn combined(result: &CommandResult) -> String {
    match (
        result.stdout.trim().is_empty(),
        result.stderr.trim().is_empty(),
    ) {
        (true, true) => String::new(),
        (false, true) => result.stdout.clone(),
        (true, false) => result.stderr.clone(),
        (false, false) => {
            format!("{}\n{}", result.stdout.trim_end(), result.stderr.trim_end())
        }
    }
}

/// The last `lines` lines of `text` — a compact tail for the run card / error
/// notice. Never panics; an empty input yields an empty string.
pub fn tail(text: &str, lines: usize) -> String {
    let rows: Vec<&str> = text.lines().collect();
    let start = rows.len().saturating_sub(lines);
    rows[start..].join("\n")
}

#[cfg(test)]
#[path = "../tests/prepare.rs"]
mod tests;
