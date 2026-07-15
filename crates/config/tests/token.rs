use super::*;

#[test]
fn generates_256_bits_of_hex() {
    let tok = generate().expect("OS RNG available");
    assert_eq!(tok.len(), 64);
    assert!(tok
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}

#[test]
fn successive_tokens_differ() {
    let a = generate().expect("OS RNG available");
    let b = generate().expect("OS RNG available");
    assert_ne!(a, b);
}

#[test]
fn hex_encodes_known_bytes() {
    assert_eq!(to_hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
    assert_eq!(to_hex(&[]), "");
}
