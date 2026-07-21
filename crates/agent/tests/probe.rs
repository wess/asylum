use super::*;

/// Pull a runnable probe's rule out of the catalog by provider key.
fn rule_of(provider: &str) -> Rule {
    match find(provider).expect("known provider").probe {
        ProbeKind::Command { rule, .. } => rule,
        ProbeKind::Unsupported(_) => panic!("{provider} has no command rule"),
    }
}

#[test]
fn aliases_resolve_case_insensitively() {
    assert_eq!(find("claude").unwrap().key, "claude");
    assert_eq!(find("Claude").unwrap().key, "claude");
    assert_eq!(find("ANTHROPIC").unwrap().key, "claude");
    assert_eq!(find("gh").unwrap().key, "github");
    assert_eq!(find("  GitHub  ".trim()).unwrap().key, "github");
    assert_eq!(find("openai").unwrap().key, "codex");
}

#[test]
fn unknown_and_empty_providers_have_no_entry() {
    assert!(find("totally-unknown").is_none());
    assert!(find("").is_none());
    assert!(find("   ").is_none());
}

#[test]
fn kind_classifies_each_provider() {
    assert!(matches!(kind("claude"), Kind::Probeable));
    assert!(matches!(kind("github"), Kind::Probeable));
    assert!(matches!(kind("codex"), Kind::Probeable));
    match kind("gemini") {
        Kind::Static(Auth::Unknown(reason)) => {
            assert_eq!(reason, "no non-interactive status command")
        }
        other => panic!("expected a static Unknown for gemini, got {other:?}"),
    }
    assert!(matches!(kind("nope"), Kind::Absent));
}

#[test]
fn claude_json_marker_decides_both_ways() {
    let rule = rule_of("claude");
    let signed_in = r#"{ "loggedIn": true, "email": "me@wess.io" }"#;
    let signed_out = r#"{ "loggedIn": false }"#;
    assert_eq!(interpret(&rule, true, signed_in, ""), Auth::SignedIn);
    assert_eq!(interpret(&rule, true, signed_out, ""), Auth::SignedOut);
    // An unrecognized-but-successful response is not guessed at.
    assert_eq!(
        interpret(&rule, true, "{}", ""),
        Auth::Unknown("could not read the CLI's sign-in state".to_string())
    );
}

#[test]
fn gh_signed_out_line_is_not_read_as_signed_in() {
    // "not logged into" contains "logged in" as a substring; the signed-out
    // marker must win over the signed-in one.
    let rule = rule_of("github");
    assert_eq!(
        interpret(
            &rule,
            false,
            "",
            "You are not logged into any GitHub hosts. Run gh auth login."
        ),
        Auth::SignedOut
    );
    assert_eq!(
        interpret(&rule, true, "  ✓ Logged in to github.com account wess", ""),
        Auth::SignedIn
    );
}

#[test]
fn codex_status_lines_are_read() {
    let rule = rule_of("codex");
    assert_eq!(
        interpret(&rule, true, "Logged in using ChatGPT", ""),
        Auth::SignedIn
    );
    assert_eq!(
        interpret(&rule, false, "Not logged in", ""),
        Auth::SignedOut
    );
}

#[test]
fn trust_exit_falls_back_to_the_exit_code() {
    // A rule that trusts the exit code decides from it when no marker matches.
    let rule = Rule {
        signed_in: &[],
        signed_out: &[],
        trust_exit: true,
    };
    assert_eq!(interpret(&rule, true, "", ""), Auth::SignedIn);
    assert_eq!(interpret(&rule, false, "", ""), Auth::SignedOut);
}

#[test]
fn without_trust_exit_an_unrecognized_result_is_unknown() {
    let rule = Rule {
        signed_in: &["yes"],
        signed_out: &["no"],
        trust_exit: false,
    };
    assert!(matches!(
        interpret(&rule, true, "something else", ""),
        Auth::Unknown(_)
    ));
}

#[test]
fn catalog_markers_are_lowercase() {
    // interpret lowercases the output, so markers must be lowercase to match.
    for p in PROVIDERS {
        if let ProbeKind::Command { rule, .. } = p.probe {
            for m in rule.signed_in.iter().chain(rule.signed_out.iter()) {
                assert!(
                    m.chars().all(|c| !c.is_uppercase()),
                    "{}: marker {m:?} is not lowercase",
                    p.key
                );
            }
        }
    }
}

#[test]
fn auth_label_and_reason() {
    assert_eq!(Auth::SignedIn.label(), "Signed in");
    assert_eq!(Auth::SignedOut.label(), "Signed out");
    assert_eq!(Auth::Unknown("nope".into()).label(), "Unknown");
    assert_eq!(Auth::SignedIn.reason(), None);
    assert_eq!(Auth::Unknown("why".into()).reason(), Some("why"));
}

#[test]
fn an_unsupported_provider_checks_to_its_reason_without_spawning() {
    assert_eq!(
        check("gemini"),
        Auth::Unknown("no non-interactive status command".to_string())
    );
}

#[test]
fn an_unknown_provider_checks_to_unknown() {
    match check("not-a-real-provider") {
        Auth::Unknown(reason) => assert!(reason.contains("no sign-in probe"), "{reason}"),
        other => panic!("expected Unknown, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn a_probe_that_would_hang_is_killed_at_the_deadline() {
    // The safety property: a command that never returns is killed and reported
    // as a timeout well before it would finish on its own.
    let start = Instant::now();
    let outcome = run("/bin/sh", &["-c", "sleep 5"], Duration::from_millis(150));
    assert!(matches!(outcome, Run::Timeout));
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "the deadline was not honored: {:?}",
        start.elapsed()
    );
}

#[cfg(unix)]
#[test]
fn output_is_captured_from_a_real_command() {
    // Exercises the spawn + pipe-drain path without a provider CLI: stdout and
    // stderr are both captured and the exit is read.
    let outcome = run(
        "/bin/sh",
        &["-c", "printf hello; printf oops 1>&2"],
        DEFAULT_TIMEOUT,
    );
    let Run::Ran {
        success,
        stdout,
        stderr,
    } = outcome
    else {
        panic!("expected the command to run to completion");
    };
    assert!(success);
    assert_eq!(stdout, "hello");
    assert_eq!(stderr, "oops");
}

#[test]
fn an_absent_provider_cli_is_reported_not_installed() {
    // check() guards on the program existing before spawning, so a provider
    // whose CLI is not on PATH reports the "not installed" reason rather than a
    // spawn error. Only asserts for providers genuinely absent on this host.
    for p in PROVIDERS {
        if let ProbeKind::Command { program, .. } = p.probe {
            if find_program(program).is_none() {
                match check(p.key) {
                    Auth::Unknown(reason) => {
                        assert!(reason.contains("is not installed"), "{reason}")
                    }
                    other => panic!("{}: expected not-installed Unknown, got {other:?}", p.key),
                }
            }
        }
    }
}

/// Live smoke test: for each provider CLI actually installed on this machine,
/// run its real status probe and require a concrete verdict (never the
/// "not installed" Unknown). Skips cleanly for absent CLIs, matching the
/// doctor's live-test style; the probe's own timeout bounds each call.
#[test]
fn installed_provider_clis_return_a_real_verdict() {
    for p in PROVIDERS {
        if let ProbeKind::Command { program, .. } = p.probe {
            if find_program(program).is_none() {
                continue;
            }
            let auth = check_with_timeout(p.key, DEFAULT_TIMEOUT);
            if let Auth::Unknown(reason) = &auth {
                assert!(
                    !reason.contains("is not installed"),
                    "{}: probe said not installed but the CLI is on PATH: {reason}",
                    p.key
                );
            }
        }
    }
}
