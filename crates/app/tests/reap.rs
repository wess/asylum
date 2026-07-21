use super::*;

#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(unix)]
use std::process::{Child, Command};
#[cfg(unix)]
use std::time::Instant;

fn temp_pidfile(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("asylumreaptest{tag}{}.pid", std::process::id()))
}

/// Spawn the wrapped argv as its own process group, the way the pty child
/// runs as its own session.
#[cfg(unix)]
fn spawn_group(argv: &[String]) -> Child {
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]).process_group(0);
    command.spawn().expect("spawn wrapped child")
}

#[cfg(unix)]
fn wait_for_pidfile(path: &Path) -> i32 {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(pid) = read(path) {
            return pid;
        }
        assert!(Instant::now() < deadline, "pidfile was never written");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn pidfile_paths_are_stable_per_run() {
    assert_eq!(pidfile(7), pidfile(7));
    assert_ne!(pidfile(7), pidfile(8));
}

#[cfg(unix)]
#[test]
fn wrap_prefixes_the_recorder_and_preserves_argv() {
    let file = temp_pidfile("wrap");
    let argv = wrap(vec!["printf".into(), "ok".into()], &file);
    assert_eq!(argv[0], "/bin/sh");
    assert_eq!(argv[1], "-c");
    assert_eq!(argv[3], file.to_string_lossy());
    assert_eq!(&argv[4..], ["printf", "ok"]);
}

#[test]
fn read_rejects_garbage_and_dangerous_pids() {
    let file = temp_pidfile("parse");
    fs::write(&file, "4242\n").unwrap();
    assert_eq!(read(&file), Some(4242));
    fs::write(&file, "nope").unwrap();
    assert_eq!(read(&file), None);
    // As group ids, 1, 0, and negatives would signal init's group, our own
    // group, or every process the user owns.
    for dangerous in ["1", "0", "-5"] {
        fs::write(&file, dangerous).unwrap();
        assert_eq!(read(&file), None);
    }
    fs::remove_file(&file).unwrap();
    assert_eq!(read(&file), None);
}

#[cfg(unix)]
#[test]
fn wrapper_records_the_exec_pid_and_terminate_ends_the_group() {
    let file = temp_pidfile("live");
    let _ = fs::remove_file(&file);
    let mut child = spawn_group(&wrap(vec!["sleep".into(), "30".into()], &file));
    let pid = wait_for_pidfile(&file);
    // `exec` keeps the wrapper shell's pid, so the file names the agent.
    assert_eq!(pid, child.id() as i32);
    terminate(&file);
    let status = child.wait().expect("wait for terminated child");
    assert!(!status.success());
    assert!(!file.exists(), "terminate consumes the pidfile");
}

#[cfg(unix)]
#[test]
fn kill_escalation_reaches_a_hup_trapping_group() {
    let file = temp_pidfile("trap");
    let ready = temp_pidfile("trapready");
    let _ = fs::remove_file(&file);
    let _ = fs::remove_file(&ready);
    // The marker file proves the trap is installed before the HUP is sent;
    // the pidfile alone is written before the exec, ahead of the trap.
    let script = format!(
        "trap '' HUP; : > '{}'; while :; do sleep 1; done",
        ready.display()
    );
    let argv = wrap(vec!["sh".into(), "-c".into(), script], &file);
    let mut child = spawn_group(&argv);
    let pid = wait_for_pidfile(&file);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready.exists() {
        assert!(Instant::now() < deadline, "the trap was never installed");
        std::thread::sleep(Duration::from_millis(10));
    }
    hangup_group(pid);
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        child.try_wait().expect("probe child").is_none(),
        "the trap should survive the polite SIGHUP"
    );
    kill_group(pid);
    let status = child.wait().expect("wait for killed child");
    assert!(!status.success());
    let _ = fs::remove_file(&file);
    let _ = fs::remove_file(&ready);
}
