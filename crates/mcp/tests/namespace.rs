use super::*;

#[test]
fn valid_service_names() {
    assert!(is_valid_service("github"));
    assert!(is_valid_service("linear-2"));
    assert!(is_valid_service("a"));
}

#[test]
fn invalid_service_names() {
    assert!(!is_valid_service(""));
    assert!(!is_valid_service("GitHub")); // no uppercase
    assert!(!is_valid_service("with space"));
    assert!(!is_valid_service("-lead"));
    assert!(!is_valid_service("trail-"));
    assert!(!is_valid_service("has__sep")); // the separator itself is barred
    assert!(!is_valid_service("under_score")); // underscore is not a slug char
}

#[test]
fn mangle_then_split_roundtrips() {
    let m = mangle("github", "create_pull_request");
    assert_eq!(m, "github__create_pull_request");
    assert_eq!(split(&m), Some(("github", "create_pull_request")));
}

#[test]
fn split_uses_the_first_separator() {
    // The underlying tool name may itself contain a `__`; the service is always
    // the segment before the first separator.
    let m = mangle("svc", "weird__tool");
    assert_eq!(split(&m), Some(("svc", "weird__tool")));
}

#[test]
fn split_rejects_unnamespaced_or_empty_halves() {
    assert_eq!(split("plainname"), None);
    assert_eq!(split("__toolonly"), None);
    assert_eq!(split("svc__"), None);
}
