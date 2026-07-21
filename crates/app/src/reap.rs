//! Deterministic termination for spawned agent process groups.
//!
//! A run's `TermView` owns its pty session behind an `Arc` that gpui clones
//! into the render tree every frame, so dropping the `run_terms` handle ends
//! the child only when the last clone happens to drop — an unpredictable
//! moment. The session API offers no by-reference kill (`Session::shutdown`
//! consumes the session), so the agent is launched through a tiny `/bin/sh`
//! wrapper that records its own pid before exec'ing the agent. `exec` keeps
//! the pid, and the pty child leads a fresh session (`setsid`), so the
//! recorded pid is also the process-group id. Ending a run signals that group
//! directly: SIGHUP first, then a group SIGKILL after a short grace period,
//! mirroring the terminal core's own teardown.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long the group SIGHUP gets before escalating to SIGKILL. Matches the
/// terminal core's teardown grace.
const GRACE: Duration = Duration::from_millis(200);

/// Where a run's pid is recorded. Only in-memory bookkeeping decides whether
/// a file is consulted, so a stale file from a crashed session is inert.
pub fn pidfile(run_id: i64) -> PathBuf {
    std::env::temp_dir().join(format!("asylumrun{run_id}.pid"))
}

/// Wrap an agent argv so the child writes its pid to `pidfile` before
/// exec'ing the agent. The write is silenced and best-effort: a failure must
/// never keep the agent from starting or leak noise into its transcript.
#[cfg(unix)]
pub fn wrap(argv: Vec<String>, pidfile: &Path) -> Vec<String> {
    let mut wrapped = Vec::with_capacity(argv.len() + 4);
    wrapped.push("/bin/sh".to_string());
    wrapped.push("-c".to_string());
    wrapped.push(r#"{ echo "$$" >"$0"; } 2>/dev/null; exec "$@""#.to_string());
    wrapped.push(pidfile.to_string_lossy().into_owned());
    wrapped.extend(argv);
    wrapped
}

#[cfg(not(unix))]
pub fn wrap(argv: Vec<String>, _pidfile: &Path) -> Vec<String> {
    argv
}

/// The pid recorded in `pidfile`, if present and sane. Pids `<= 1` are
/// rejected: as a group id, 0 would signal our own group and -1 every
/// process the user owns.
fn read(pidfile: &Path) -> Option<i32> {
    fs::read_to_string(pidfile)
        .ok()?
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|pid| *pid > 1)
}

/// End one run's process group: SIGHUP now, group SIGKILL after the grace
/// period (on a detached thread, so the caller never blocks). Consumes the
/// pidfile; a missing or malformed file is a no-op.
pub fn terminate(pidfile: &Path) {
    let pid = read(pidfile);
    let _ = fs::remove_file(pidfile);
    let Some(pid) = pid else {
        return;
    };
    hangup_group(pid);
    std::thread::spawn(move || {
        std::thread::sleep(GRACE);
        kill_group(pid);
    });
}

/// End every listed process group before the process exits: SIGHUP all, one
/// shared grace period, SIGKILL all. Synchronous, for the quit path — a
/// detached escalation thread would not survive it.
pub fn terminate_all(pidfiles: Vec<PathBuf>) {
    let pids: Vec<i32> = pidfiles
        .iter()
        .filter_map(|path| {
            let pid = read(path);
            let _ = fs::remove_file(path);
            pid
        })
        .collect();
    if pids.is_empty() {
        return;
    }
    for pid in &pids {
        hangup_group(*pid);
    }
    std::thread::sleep(GRACE);
    for pid in &pids {
        kill_group(*pid);
    }
}

// A negative pid addresses the whole process group; ESRCH (already gone) is
// expected on the escalation path and ignored.

#[cfg(unix)]
fn hangup_group(pid: i32) {
    unsafe {
        libc::kill(-pid, libc::SIGHUP);
    }
}

#[cfg(unix)]
fn kill_group(pid: i32) {
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn hangup_group(_pid: i32) {}

#[cfg(not(unix))]
fn kill_group(_pid: i32) {}

#[cfg(test)]
#[path = "../tests/reap.rs"]
mod tests;
