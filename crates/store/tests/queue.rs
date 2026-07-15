use super::*;

#[test]
fn backoff_grows_then_caps() {
    // Exponential from the base, capped so retries never park too long.
    assert_eq!(backoff_secs(1), 5);
    assert_eq!(backoff_secs(2), 10);
    assert_eq!(backoff_secs(3), 20);
    assert_eq!(backoff_secs(4), 40);
    assert_eq!(backoff_secs(5), 80);
    assert_eq!(backoff_secs(6), 160);
    // 320 would exceed the cap.
    assert_eq!(backoff_secs(7), 300);
    assert_eq!(backoff_secs(100), 300);
}

#[test]
fn backoff_treats_zero_and_negative_as_first_attempt() {
    assert_eq!(backoff_secs(0), 5);
    assert_eq!(backoff_secs(-3), 5);
}
