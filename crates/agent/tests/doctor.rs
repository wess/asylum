use super::*;

#[test]
fn empty_and_missing_programs_are_not_ready() {
    assert_eq!(find_program(""), None);
    assert_eq!(find_program("asylum-program-that-does-not-exist"), None);
}

#[cfg(unix)]
#[test]
fn absolute_executable_is_ready() {
    assert_eq!(
        find_program("/bin/sh"),
        Some(std::path::PathBuf::from("/bin/sh"))
    );
}

#[test]
fn a_version_line_is_the_first_thing_it_printed() {
    assert_eq!(
        classify("claude", true, "claude 1.2.3\nextra\n", ""),
        Probe::Ok("claude 1.2.3".to_string())
    );
}

#[test]
fn leading_blank_lines_are_skipped() {
    assert_eq!(
        classify("codex", true, "\n\n  codex 0.4  \n", ""),
        Probe::Ok("codex 0.4".to_string())
    );
}

#[test]
fn a_silent_success_still_counts_as_ok() {
    assert_eq!(
        classify("aider", true, "   \n", ""),
        Probe::Ok("ok".to_string())
    );
}

#[test]
fn a_failure_reports_what_stderr_said() {
    assert_eq!(
        classify("gemini", false, "", "unknown flag: --version\n"),
        Probe::Failed("unknown flag: --version".to_string())
    );
}

#[test]
fn a_silent_failure_names_the_command() {
    assert_eq!(
        classify("gemini", false, "", ""),
        Probe::Failed("`gemini --version` failed".to_string())
    );
}

#[test]
fn an_empty_program_is_missing_rather_than_spawned() {
    assert!(matches!(probe("   "), Probe::Missing(_)));
}

#[test]
fn a_program_not_on_path_is_missing() {
    match probe("asylum-program-that-does-not-exist") {
        Probe::Missing(m) => assert!(m.contains("not found on PATH"), "{m}"),
        other => panic!("expected Missing, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn a_real_executable_is_probed_for_real() {
    // Covers the spawn path rather than the reporting rules. What `true`
    // answers `--version` with is the platform's business: GNU coreutils
    // prints a version line, macOS prints nothing and lands on the
    // silent-success path, and both are Ok.
    assert!(probe("/usr/bin/true").ok());
}

#[test]
fn only_ok_counts_as_ok() {
    assert!(Probe::Ok("v1".into()).ok());
    assert!(!Probe::Failed("boom".into()).ok());
    assert!(!Probe::Missing("gone".into()).ok());
    assert_eq!(Probe::Failed("boom".into()).message(), "boom");
}
