use super::*;

use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn scratch() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("asylumprep{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cfg(setup: &[&str]) -> config::ProjectConfig {
    config::ProjectConfig {
        setup: setup.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

fn result(command: &str, disposition: Disposition, stdout: &str, stderr: &str) -> CommandResult {
    CommandResult {
        command: command.to_string(),
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
        disposition,
        duration_ms: 1234,
    }
}

// --- pure: tail truncation ---

#[test]
fn tail_returns_all_when_fewer_lines_than_requested() {
    assert_eq!(tail("a\nb", 5), "a\nb");
}

#[test]
fn tail_keeps_only_the_last_lines() {
    assert_eq!(tail("a\nb\nc\nd", 2), "c\nd");
}

#[test]
fn tail_of_empty_is_empty() {
    assert_eq!(tail("", 4), "");
}

// --- pure: report outcome derivation ---

#[test]
fn empty_report_is_ok() {
    assert_eq!(SetupReport::default().outcome(), SetupOutcome::Ok);
}

#[test]
fn all_ok_report_is_ok() {
    let report = SetupReport {
        results: vec![
            result("a", Disposition::Ok, "", ""),
            result("b", Disposition::Ok, "", ""),
        ],
    };
    assert_eq!(report.outcome(), SetupOutcome::Ok);
}

#[test]
fn last_failed_report_is_failed() {
    let report = SetupReport {
        results: vec![
            result("a", Disposition::Ok, "", ""),
            result("b", Disposition::Failed(1), "", "boom"),
        ],
    };
    assert_eq!(report.outcome(), SetupOutcome::Failed);
}

#[test]
fn timed_out_report_is_failed() {
    let report = SetupReport {
        results: vec![result("a", Disposition::TimedOut, "", "")],
    };
    assert_eq!(report.outcome(), SetupOutcome::Failed);
}

#[test]
fn cancelled_report_is_cancelled() {
    let report = SetupReport {
        results: vec![result("a", Disposition::Cancelled, "", "")],
    };
    assert_eq!(report.outcome(), SetupOutcome::Cancelled);
}

// --- pure: failure_message ---

#[test]
fn failure_message_names_command_and_exit_code() {
    let report = SetupReport {
        results: vec![result(
            "bun install",
            Disposition::Failed(1),
            "",
            "npm error missing",
        )],
    };
    let message = failure_message(&report).expect("a failed report yields a message");
    assert!(
        message.contains("Setup command failed: bun install (exit 1)"),
        "{message}"
    );
    assert!(message.contains("npm error missing"), "{message}");
}

#[test]
fn failure_message_falls_back_to_stdout_when_stderr_empty() {
    let report = SetupReport {
        results: vec![result(
            "bun run build",
            Disposition::Failed(2),
            "compile error here",
            "",
        )],
    };
    let message = failure_message(&report).unwrap();
    assert!(message.contains("compile error here"), "{message}");
}

#[test]
fn failure_message_reports_timeout() {
    let report = SetupReport {
        results: vec![result("sleep 999", Disposition::TimedOut, "", "")],
    };
    let message = failure_message(&report).unwrap();
    assert!(message.contains("timed out"), "{message}");
    assert!(message.contains("sleep 999"), "{message}");
}

#[test]
fn failure_message_reports_unstartable() {
    let report = SetupReport {
        results: vec![result(
            "weirdcmd",
            Disposition::Unstartable,
            "",
            "could not run `weirdcmd`",
        )],
    };
    let message = failure_message(&report).unwrap();
    assert!(message.contains("could not start"), "{message}");
}

#[test]
fn failure_message_is_none_for_ok() {
    let report = SetupReport {
        results: vec![result("a", Disposition::Ok, "done", "")],
    };
    assert!(failure_message(&report).is_none());
}

#[test]
fn failure_message_is_none_for_cancelled() {
    let report = SetupReport {
        results: vec![result("a", Disposition::Cancelled, "", "")],
    };
    assert!(failure_message(&report).is_none());
}

// --- pure: transcript ---

#[test]
fn transcript_has_a_section_per_command() {
    let report = SetupReport {
        results: vec![
            result("bun install", Disposition::Ok, "installed 1 package", ""),
            result("bun run build", Disposition::Failed(1), "", "build failed"),
        ],
    };
    let text = transcript(&report);
    assert!(text.contains("$ bun install"), "{text}");
    assert!(text.contains("[ok,"), "{text}");
    assert!(text.contains("installed 1 package"), "{text}");
    assert!(text.contains("$ bun run build"), "{text}");
    assert!(text.contains("[exit 1,"), "{text}");
    assert!(text.contains("build failed"), "{text}");
}

// --- process-level: running real commands ---

#[cfg(unix)]
fn no_cancel() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

#[cfg(unix)]
#[test]
fn run_executes_command_and_succeeds() {
    let dir = scratch();
    let report = run(
        &dir,
        &cfg(&["echo hello"]),
        config::Trust::Trusted,
        &no_cancel(),
        Duration::from_secs(30),
    );
    assert_eq!(report.outcome(), SetupOutcome::Ok);
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].disposition, Disposition::Ok);
    assert!(report.results[0].stdout.contains("hello"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn run_stops_at_first_failure_and_records_exit_code() {
    let dir = scratch();
    let report = run(
        &dir,
        &cfg(&["exit 3", "echo should-not-run"]),
        config::Trust::Trusted,
        &no_cancel(),
        Duration::from_secs(30),
    );
    assert_eq!(report.outcome(), SetupOutcome::Failed);
    // The second command never ran — the sequence stopped at the failure.
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].disposition, Disposition::Failed(3));
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn run_captures_stdout_and_stderr_separately() {
    let dir = scratch();
    let report = run(
        &dir,
        &cfg(&["echo out; echo err 1>&2"]),
        config::Trust::Trusted,
        &no_cancel(),
        Duration::from_secs(30),
    );
    let first = &report.results[0];
    assert!(first.stdout.contains("out"), "stdout: {}", first.stdout);
    assert!(first.stderr.contains("err"), "stderr: {}", first.stderr);
    assert!(
        !first.stdout.contains("err"),
        "stdout leaked stderr: {}",
        first.stdout
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn run_kills_on_timeout() {
    let dir = scratch();
    let start = Instant::now();
    let report = run(
        &dir,
        &cfg(&["sleep 5"]),
        config::Trust::Trusted,
        &no_cancel(),
        Duration::from_millis(200),
    );
    assert_eq!(report.outcome(), SetupOutcome::Failed);
    assert_eq!(report.results[0].disposition, Disposition::TimedOut);
    // Killed promptly, nowhere near the full 5s sleep.
    assert!(
        start.elapsed() < Duration::from_secs(4),
        "took {:?}",
        start.elapsed()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn run_cancel_kills_the_running_command() {
    let dir = scratch();
    let cancel = no_cancel();
    let flag = cancel.clone();
    let killer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        flag.store(true, Ordering::Relaxed);
    });
    let start = Instant::now();
    let report = run(
        &dir,
        &cfg(&["sleep 5"]),
        config::Trust::Trusted,
        &cancel,
        Duration::from_secs(30),
    );
    killer.join().unwrap();
    assert_eq!(report.outcome(), SetupOutcome::Cancelled);
    assert_eq!(report.results[0].disposition, Disposition::Cancelled);
    assert!(
        start.elapsed() < Duration::from_secs(4),
        "took {:?}",
        start.elapsed()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn run_cancelled_before_start_runs_nothing() {
    let dir = scratch();
    let cancel = Arc::new(AtomicBool::new(true));
    let report = run(
        &dir,
        &cfg(&["echo nope"]),
        config::Trust::Trusted,
        &cancel,
        Duration::from_secs(30),
    );
    assert_eq!(report.outcome(), SetupOutcome::Cancelled);
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].disposition, Disposition::Cancelled);
    assert!(report.results[0].stdout.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Repository trust ────────────────────────────────────────────────────────
//
// Setup is the one place that hands repository-authored text to a shell, so
// these assert the *side effect*, not just the report: an untrusted project
// must leave no evidence that its command ever ran.

#[cfg(unix)]
#[test]
fn untrusted_project_does_not_execute_setup() {
    let dir = scratch();
    let marker = dir.join("executed-untrusted");
    let _ = std::fs::remove_file(&marker);
    let command = format!("touch {}", marker.display());

    let report = run(
        &dir,
        &cfg(&[command.as_str()]),
        config::Trust::Untrusted,
        &no_cancel(),
        Duration::from_secs(30),
    );

    // The security property.
    assert!(
        !marker.exists(),
        "an untrusted repository's setup command executed"
    );
    // And it is recorded rather than silently dropped, so the UI can say what
    // was withheld and offer the trust decision.
    assert_eq!(report.outcome(), SetupOutcome::Withheld);
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].disposition, Disposition::Untrusted);
    assert_eq!(report.results[0].command, command);
}

#[cfg(unix)]
#[test]
fn trusted_project_executes_the_same_command() {
    // The control: without it, the test above could pass because the command
    // was broken rather than because trust withheld it.
    let dir = scratch();
    let marker = dir.join("executed-trusted");
    let _ = std::fs::remove_file(&marker);
    let command = format!("touch {}", marker.display());

    let report = run(
        &dir,
        &cfg(&[command.as_str()]),
        config::Trust::Trusted,
        &no_cancel(),
        Duration::from_secs(30),
    );

    assert_eq!(report.outcome(), SetupOutcome::Ok);
    assert!(marker.exists(), "a trusted repository's setup did not run");
}

#[cfg(unix)]
#[test]
fn untrusted_withholds_every_command_not_just_the_first() {
    let dir = scratch();
    let first = dir.join("first");
    let second = dir.join("second");
    let commands = [
        format!("touch {}", first.display()),
        format!("touch {}", second.display()),
    ];

    let report = run(
        &dir,
        &cfg(&[commands[0].as_str(), commands[1].as_str()]),
        config::Trust::Untrusted,
        &no_cancel(),
        Duration::from_secs(30),
    );

    assert!(!first.exists() && !second.exists());
    assert_eq!(report.results.len(), 2);
    assert!(report
        .results
        .iter()
        .all(|r| r.disposition == Disposition::Untrusted));
}

#[test]
fn withheld_is_not_reported_as_a_failure() {
    let report = SetupReport {
        results: vec![result("bun install", Disposition::Untrusted, "", "")],
    };
    assert_eq!(report.outcome(), SetupOutcome::Withheld);
    // Nothing went wrong, so the failure channel stays quiet.
    assert!(failure_message(&report).is_none());
    // The dedicated channel names the command, because "do you want this to
    // run" is not answerable without seeing it.
    let message = withheld_message(&report).expect("withheld report explains itself");
    assert!(message.contains("bun install"), "message was: {message}");
}

#[test]
fn no_withheld_message_when_nothing_was_withheld() {
    let report = SetupReport {
        results: vec![result("bun install", Disposition::Ok, "done", "")],
    };
    assert!(withheld_message(&report).is_none());
}
