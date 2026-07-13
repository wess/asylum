use super::*;
use agent::SpawnSpec;
use std::time::Duration;

fn spec(program: &str, args: &[&str]) -> SpawnSpec {
    SpawnSpec {
        program: program.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        cwd: std::env::temp_dir().to_string_lossy().into_owned(),
        stdin: None,
    }
}

#[test]
fn state_helpers() {
    assert!(State::Exited(Some(0)).succeeded());
    assert!(!State::Exited(Some(1)).succeeded());
    assert!(State::Exited(None).is_terminal());
    assert!(!State::Running.is_terminal());
    assert_eq!(State::Exited(Some(3)).exit_code(), Some(3));
}

#[cfg(unix)]
#[test]
fn runs_to_success() {
    let run = Runner::start(&spec("true", &[])).expect("spawn true");
    let state = run.wait(Duration::from_secs(5));
    assert_eq!(state, State::Exited(Some(0)));
    assert!(state.succeeded());
    run.shutdown();
}

#[cfg(unix)]
#[test]
fn nonzero_exit_is_captured() {
    let run = Runner::start(&spec("sh", &["-c", "exit 7"])).expect("spawn sh");
    let state = run.wait(Duration::from_secs(5));
    assert_eq!(state, State::Exited(Some(7)));
    assert!(!state.succeeded());
    run.shutdown();
}

#[cfg(unix)]
#[test]
fn captures_output_text() {
    let run = Runner::start(&spec("sh", &["-c", "echo asylum-marker; sleep 0.2"]))
        .expect("spawn sh");
    // Give the pty a moment to render, then snapshot before it exits.
    std::thread::sleep(Duration::from_millis(120));
    let text = run.screen_text();
    assert!(text.contains("asylum-marker"), "screen was: {text:?}");
    run.wait(Duration::from_secs(5));
    run.shutdown();
}

#[test]
fn scrollback_save_load_clear_roundtrip() {
    let path = std::env::temp_dir()
        .join(format!("asylum-sb-{}", std::process::id()))
        .join("run.scrollback");
    scrollback::save(&path, "line one\nline two\n").unwrap();
    assert_eq!(scrollback::load(&path).as_deref(), Some("line one\nline two\n"));
    scrollback::clear(&path).unwrap();
    assert!(scrollback::load(&path).is_none());
    // Clearing a missing file is not an error.
    scrollback::clear(&path).unwrap();
}

#[cfg(unix)]
#[test]
fn history_persists_across_a_simulated_restart() {
    let run = Runner::start(&spec("sh", &["-c", "echo persist-me; sleep 0.2"])).unwrap();
    std::thread::sleep(Duration::from_millis(120));
    let history = run.history_text();
    assert!(history.contains("persist-me"));

    let path = std::env::temp_dir()
        .join(format!("asylum-sb2-{}", std::process::id()))
        .join("run.scrollback");
    scrollback::save(&path, &history).unwrap();
    run.wait(Duration::from_secs(5));
    run.shutdown();

    // "Restart": the text is available without the original run.
    let restored = scrollback::load(&path).unwrap();
    assert!(restored.contains("persist-me"));
    let _ = std::fs::remove_file(&path);
}

#[cfg(unix)]
#[test]
fn still_running_reports_running() {
    let run = Runner::start(&spec("sh", &["-c", "sleep 1"])).expect("spawn sh");
    assert!(run.is_running());
    // A short wait times out while it is still sleeping.
    let state = run.wait(Duration::from_millis(80));
    assert_eq!(state, State::Running);
    run.shutdown();
}
