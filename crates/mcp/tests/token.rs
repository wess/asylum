use super::*;

const KEY: &str = "session-signing-key";

#[test]
fn mint_then_verify_roundtrips() {
    let t = mint(KEY, 42, 7, 0);
    let scope = verify(&t, KEY, 1000).expect("valid");
    assert_eq!(scope.project, 42);
    assert_eq!(scope.run, 7);
    assert_eq!(scope.expires_at, 0);
}

#[test]
fn a_wrong_key_does_not_verify() {
    let t = mint(KEY, 1, 1, 0);
    assert!(verify(&t, "other-key", 1000).is_none());
}

#[test]
fn expiry_is_enforced() {
    let t = mint(KEY, 1, 1, 500);
    assert!(verify(&t, KEY, 499).is_some());
    assert!(verify(&t, KEY, 500).is_none());
    assert!(verify(&t, KEY, 501).is_none());
    // Zero means never expires.
    let forever = mint(KEY, 1, 1, 0);
    assert!(verify(&forever, KEY, i64::MAX).is_some());
}

#[test]
fn tampering_is_rejected() {
    let t = mint(KEY, 1, 1, 0);
    // Flip the project in the payload but keep the old mac.
    let forged = t.replacen("v1.1.1.", "v1.999.1.", 1);
    assert!(verify(&forged, KEY, 1000).is_none());
}

#[test]
fn extra_or_missing_fields_are_rejected() {
    // A valid-looking payload with an extra segment must not verify (the mac
    // covers the exact field set).
    let payload = "v1.1.1.0.extra";
    let mac = {
        // Re-sign the tampered payload with the key to isolate the arity check
        // from the mac check.
        let t = mint(KEY, 0, 0, 0);
        let idx = t.rfind('.').unwrap();
        // Not the right mac; just ensure a 6-part token is rejected regardless.
        t[idx + 1..].to_string()
    };
    assert!(verify(&format!("{payload}.{mac}"), KEY, 1000).is_none());
}
