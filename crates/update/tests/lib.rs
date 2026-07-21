use super::*;

#[test]
fn curl_args_carry_deadlines_and_size_cap() {
    let args = curl_args("https://api.github.com/repos/a/b/releases/latest");
    assert!(args.iter().any(|a| a == "--connect-timeout"));
    assert!(args.iter().any(|a| a == "--max-time"));
    assert!(args.iter().any(|a| a == "--max-filesize"));
    assert_eq!(
        args.last().unwrap(),
        "https://api.github.com/repos/a/b/releases/latest"
    );
}

#[test]
fn repo_slugs_are_validated() {
    assert!(is_valid_repo("wess/asylum"));
    assert!(is_valid_repo("owner-1/repo.name_2"));
    assert!(!is_valid_repo("no-slash"));
    assert!(!is_valid_repo("a/b/c"));
    assert!(!is_valid_repo("../etc/passwd"));
    assert!(!is_valid_repo("a/../b"));
    assert!(!is_valid_repo("a b/c"));
    assert!(!is_valid_repo(""));
    assert!(fetch_latest("bad slug/with spaces").is_err());
}

#[test]
fn parses_and_orders_versions() {
    assert_eq!(Version::parse("v0.1.0"), Version::parse("0.1.0"));
    assert_eq!(
        Version::parse("1"),
        Some(Version {
            major: 1,
            minor: 0,
            patch: 0
        })
    );
    assert_eq!(
        Version::parse("2.3"),
        Some(Version {
            major: 2,
            minor: 3,
            patch: 0
        })
    );
    // Pre-release and build suffixes are ignored.
    assert_eq!(Version::parse("1.2.3-rc.1"), Version::parse("1.2.3"));
    assert_eq!(Version::parse("1.2.3+build"), Version::parse("1.2.3"));
    assert!(Version::parse("").is_none());
    assert!(Version::parse("not-a-version").is_none());

    assert!(Version::parse("0.2.0").unwrap() > Version::parse("0.1.9").unwrap());
    assert!(Version::parse("1.0.0").unwrap() > Version::parse("0.99.99").unwrap());
}

#[test]
fn evaluate_flags_a_newer_release() {
    let json = r#"{"tag_name":"v0.2.0","html_url":"https://example.com/rel","body":"Notes","draft":false,"prerelease":false}"#;
    match evaluate("0.1.0", json) {
        Status::Available(release) => {
            assert_eq!(release.tag, "v0.2.0");
            assert_eq!(
                release.version,
                Version {
                    major: 0,
                    minor: 2,
                    patch: 0
                }
            );
            assert_eq!(release.url, "https://example.com/rel");
            assert_eq!(release.notes, "Notes");
        }
        other => panic!("expected Available, got {other:?}"),
    }
}

#[test]
fn evaluate_is_uptodate_when_current_or_newer() {
    let same = r#"{"tag_name":"0.1.0"}"#;
    assert_eq!(evaluate("0.1.0", same), Status::UpToDate);
    let older = r#"{"tag_name":"0.0.9"}"#;
    assert_eq!(evaluate("0.1.0", older), Status::UpToDate);
}

#[test]
fn evaluate_ignores_drafts_and_prereleases() {
    let draft = r#"{"tag_name":"v9.9.9","draft":true}"#;
    assert_eq!(evaluate("0.1.0", draft), Status::UpToDate);
    let pre = r#"{"tag_name":"v9.9.9","prerelease":true}"#;
    assert_eq!(evaluate("0.1.0", pre), Status::UpToDate);
}

#[test]
fn evaluate_unknown_on_garbage() {
    assert_eq!(evaluate("0.1.0", "not json"), Status::Unknown);
    assert_eq!(evaluate("bad", r#"{"tag_name":"1.0.0"}"#), Status::Unknown);
    assert_eq!(evaluate("0.1.0", r#"{"tag_name":"nope"}"#), Status::Unknown);
}

#[test]
fn release_captures_name_published_at_and_url() {
    let json = r#"{"tag_name":"v1.4.0","name":"Widgets galore","html_url":"https://example.com/r/1.4.0","published_at":"2026-01-02T03:04:05Z","body":"Added widgets.","draft":false,"prerelease":false}"#;
    match evaluate("1.0.0", json) {
        Status::Available(release) => {
            assert_eq!(release.name, "Widgets galore");
            assert_eq!(release.published_at, "2026-01-02T03:04:05Z");
            assert_eq!(release.url, "https://example.com/r/1.4.0");
            assert_eq!(release.notes, "Added widgets.");
        }
        other => panic!("expected Available, got {other:?}"),
    }
}

#[test]
fn release_defaults_missing_optional_fields() {
    // Only tag_name present: name/html_url/published_at/body all default to
    // empty rather than failing to parse.
    let json = r#"{"tag_name":"v2.0.0"}"#;
    match evaluate("1.0.0", json) {
        Status::Available(release) => {
            assert_eq!(release.name, "");
            assert_eq!(release.published_at, "");
            assert_eq!(release.url, "");
            assert_eq!(release.notes, "");
        }
        other => panic!("expected Available, got {other:?}"),
    }
}

#[test]
fn notes_within_cap_are_untouched() {
    let short = "Fixed a bug.";
    assert_eq!(truncate_notes(short), short);
}

#[test]
fn notes_over_cap_are_truncated_with_a_note() {
    let long = "a".repeat(NOTES_CAP + 500);
    let truncated = truncate_notes(&long);
    assert!(truncated.len() < long.len());
    assert!(truncated.ends_with("…\n\n(truncated)"));
    assert!(truncated.starts_with(&"a".repeat(NOTES_CAP)));
}

#[test]
fn notes_truncation_is_char_boundary_safe() {
    // Multi-byte characters straddling the cap must not panic.
    let long = "€".repeat(NOTES_CAP);
    let truncated = truncate_notes(&long);
    assert!(truncated.ends_with("…\n\n(truncated)"));
}
