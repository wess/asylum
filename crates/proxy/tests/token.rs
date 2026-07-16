use super::*;

#[test]
fn mint_and_verify_round_trip() {
    assert_eq!(verify(&mint("key", 7, 0), "key", 1000), Some(7));
    assert_eq!(verify(&mint("key", 0, 0), "key", 1000), Some(0));
}

#[test]
fn wrong_key_or_tamper_rejected() {
    let tok = mint("key", 7, 0);
    assert_eq!(verify(&tok, "other", 1000), None);
    let bad = tok.replacen("v1.7.", "v1.9.", 1);
    assert_eq!(verify(&bad, "key", 1000), None);
}

#[test]
fn expiry_enforced() {
    let tok = mint("key", 1, 500);
    assert_eq!(verify(&tok, "key", 499), Some(1));
    assert_eq!(verify(&tok, "key", 500), None);
    assert_eq!(verify(&mint("key", 1, 0), "key", i64::MAX), Some(1));
}

#[test]
fn garbage_rejected() {
    assert_eq!(verify("", "k", 0), None);
    assert_eq!(verify("nope", "k", 0), None);
    assert_eq!(verify("v1.1.0", "k", 0), None); // no mac segment
    assert_eq!(verify("v2.1.0.deadbeef", "k", 0), None); // wrong version
}
