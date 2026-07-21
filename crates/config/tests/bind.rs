use super::*;

#[test]
fn loopback_ipv4_is_loopback_only() {
    assert!(is_loopback_only("127.0.0.1:8788"));
    assert!(is_loopback_only("127.0.0.1:0"));
}

#[test]
fn loopback_ipv6_is_loopback_only() {
    assert!(is_loopback_only("[::1]:8788"));
}

#[test]
fn localhost_name_is_loopback_only() {
    // localhost resolves to 127.0.0.1 and/or ::1; both are loopback.
    assert!(is_loopback_only("localhost:8788"));
}

#[test]
fn wildcard_ipv4_is_not_loopback() {
    assert!(!is_loopback_only("0.0.0.0:8787"));
}

#[test]
fn wildcard_ipv6_is_not_loopback() {
    assert!(!is_loopback_only("[::]:8787"));
}

#[test]
fn unresolvable_is_not_loopback() {
    assert!(!is_loopback_only("not a socket addr"));
    assert!(!is_loopback_only("missing-port"));
}

#[test]
fn token_required_refuses_empty_token_on_ipv4_loopback() {
    assert_eq!(
        guard("127.0.0.1:8787", "", Policy::TokenRequired),
        Err(Refusal::MissingToken("127.0.0.1:8787".into()))
    );
    // Whitespace-only is treated the same as empty.
    assert_eq!(
        guard("127.0.0.1:8787", "   ", Policy::TokenRequired),
        Err(Refusal::MissingToken("127.0.0.1:8787".into()))
    );
}

#[test]
fn token_required_refuses_empty_token_on_ipv6_loopback() {
    assert_eq!(
        guard("[::1]:8787", "", Policy::TokenRequired),
        Err(Refusal::MissingToken("[::1]:8787".into()))
    );
}

#[test]
fn token_required_allows_loopback_with_a_token() {
    assert_eq!(
        guard("127.0.0.1:8787", "s3cret", Policy::TokenRequired),
        Ok(())
    );
    assert_eq!(guard("[::1]:8787", "s3cret", Policy::TokenRequired), Ok(()));
}

#[test]
fn token_required_refuses_wildcard_without_token() {
    assert_eq!(
        guard("0.0.0.0:8787", "", Policy::TokenRequired),
        Err(Refusal::MissingToken("0.0.0.0:8787".into()))
    );
    assert_eq!(
        guard("[::]:8787", "", Policy::TokenRequired),
        Err(Refusal::MissingToken("[::]:8787".into()))
    );
}

#[test]
fn token_required_allows_wildcard_with_token() {
    assert_eq!(
        guard("0.0.0.0:8787", "s3cret", Policy::TokenRequired),
        Ok(())
    );
}

#[test]
fn missing_token_refusal_names_the_settings_key_and_env_var() {
    let message = guard("127.0.0.1:8787", "", Policy::TokenRequired)
        .unwrap_err()
        .to_string();
    assert!(message.contains("companion.token"), "message: {message}");
    assert!(
        message.contains("ASYLUM_COMPANION_TOKEN"),
        "message: {message}"
    );
}

#[test]
fn loopback_only_refuses_wildcard_even_with_token() {
    assert_eq!(
        guard("0.0.0.0:8788", "s3cret", Policy::LoopbackOnly),
        Err(Refusal::NonLoopbackNotAllowed("0.0.0.0:8788".into()))
    );
    assert_eq!(guard("127.0.0.1:8788", "", Policy::LoopbackOnly), Ok(()));
}

#[test]
fn unresolvable_bind_is_refused_under_any_policy() {
    assert_eq!(
        guard("nonsense", "tok", Policy::TokenRequired),
        Err(Refusal::Unresolvable("nonsense".into()))
    );
    assert_eq!(
        guard("nonsense", "tok", Policy::LoopbackOnly),
        Err(Refusal::Unresolvable("nonsense".into()))
    );
}
