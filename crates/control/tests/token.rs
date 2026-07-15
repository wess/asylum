use super::*;

#[test]
fn mint_and_verify_round_trip() {
    let key = "sessionkey";
    let tok = mint(key, 7, 42, 0);
    let scope = verify(&tok, key, 1000).expect("valid");
    assert_eq!(scope.task_id, 7);
    assert_eq!(scope.run_id, 42);
    assert_eq!(scope.expires_at, 0);
}

#[test]
fn a_tampered_payload_is_rejected() {
    let key = "sessionkey";
    let tok = mint(key, 7, 42, 0);
    // Swap the task id in the payload but keep the old MAC.
    let mangled = tok.replacen("v1.7.", "v1.9.", 1);
    assert!(verify(&mangled, key, 1000).is_none());
}

#[test]
fn a_wrong_key_is_rejected() {
    let tok = mint("keyA", 1, 1, 0);
    assert!(verify(&tok, "keyB", 1000).is_none());
}

#[test]
fn garbage_tokens_are_rejected() {
    let key = "k";
    assert!(verify("", key, 0).is_none());
    assert!(verify("not-a-token", key, 0).is_none());
    assert!(verify("v1.1.1.0", key, 0).is_none()); // no MAC segment
    assert!(verify("v2.1.1.0.deadbeef", key, 0).is_none()); // wrong version
}

#[test]
fn expiry_is_enforced() {
    let key = "k";
    let tok = mint(key, 1, 1, 500);
    // Before expiry: valid.
    assert!(verify(&tok, key, 499).is_some());
    // At/after expiry: rejected.
    assert!(verify(&tok, key, 500).is_none());
    assert!(verify(&tok, key, 600).is_none());
    // Zero expiry never expires.
    let forever = mint(key, 1, 1, 0);
    assert!(verify(&forever, key, i64::MAX).is_some());
}
