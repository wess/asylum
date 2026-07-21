//! Pure-logic tests for incremental transcript persistence: change detection,
//! the cadence decision, and the storage cap with its truncation note.

use super::*;

fn mark_from(text: &str, saved_at: i64) -> Mark {
    let (len, hash) = fingerprint(text);
    Mark {
        saved_at,
        len,
        hash,
    }
}

#[test]
fn char_boundary_rounds_up_to_the_next_code_point() {
    // "a⠋b": a=1 byte, ⠋=3 bytes (1..4), b=1 byte.
    let s = "a⠋b";
    assert_eq!(char_boundary(s, 0), 0);
    assert_eq!(char_boundary(s, 1), 1); // start of ⠋
    assert_eq!(char_boundary(s, 2), 4); // mid ⠋ → next boundary
    assert_eq!(char_boundary(s, 3), 4);
    assert_eq!(char_boundary(s, 4), 4); // start of b
    assert_eq!(char_boundary(s, 5), 5);
}

#[test]
fn cap_leaves_text_within_budget_untouched() {
    assert_eq!(cap("short"), "short");
    let exact = "x".repeat(BYTE_CAP);
    assert_eq!(cap(&exact), exact);
}

#[test]
fn cap_trims_to_the_tail_behind_a_note() {
    let text = "x".repeat(BYTE_CAP + 10);
    let out = cap(&text);
    assert!(out.starts_with("[earlier terminal output trimmed;"));
    let (_, tail) = out.split_once('\n').expect("note is its own line");
    assert_eq!(tail.len(), BYTE_CAP);
    assert!(tail.chars().all(|c| c == 'x'));
}

#[test]
fn cap_never_splits_a_multibyte_char() {
    // ⠋ is three bytes, and BYTE_CAP is not a multiple of three, so the cut
    // lands mid-code-point unless it is rounded to a boundary.
    let text = "⠋".repeat(BYTE_CAP);
    let out = cap(&text);
    assert!(out.starts_with("[earlier terminal output trimmed;"));
    let (_, tail) = out.split_once('\n').expect("note is its own line");
    assert!(tail.len() <= BYTE_CAP);
    assert!(tail.chars().all(|c| c == '⠋'));
    assert!(out.ends_with('⠋'));
}

#[test]
fn first_snapshot_always_persists() {
    assert!(should_persist(None, "any output", 100));
}

#[test]
fn persist_waits_for_the_cadence_even_when_changed() {
    let mark = mark_from("old output", 100);
    // Different text, but the cadence has not elapsed yet.
    assert!(!should_persist(
        Some(mark),
        "brand new output",
        100 + CADENCE_SECS - 1
    ));
}

#[test]
fn persist_skips_an_unchanged_transcript() {
    let mark = mark_from("identical output", 100);
    assert!(!should_persist(
        Some(mark),
        "identical output",
        100 + CADENCE_SECS * 3
    ));
}

#[test]
fn persist_writes_once_changed_and_the_cadence_elapsed() {
    let mark = mark_from("old output", 100);
    // Exactly at the cadence boundary counts (>=).
    assert!(should_persist(
        Some(mark),
        "old output and then some",
        100 + CADENCE_SECS
    ));
}

#[test]
fn change_detection_catches_appends() {
    let mark = mark_from("hello", 0);
    assert!(!changed(&mark, "hello"));
    assert!(changed(&mark, "hello world"));
}

#[test]
fn change_detection_catches_a_tail_turnover_at_a_steady_length() {
    // A full scrollback shifts up by a line: the length holds but the tail
    // turns over. The tail hash catches it where a length check alone would not.
    let mark = mark_from("aaaabbbb", 0);
    assert!(changed(&mark, "aaaacccc"));
}

#[test]
fn change_detection_is_scoped_to_the_tail() {
    // Documented tradeoff: a change far above the tail window, at an identical
    // length and identical tail, reads as unchanged. Real terminal edits move
    // the tail or the length, so this is acceptable for a recovery snapshot.
    let tail = "t".repeat(TAIL_HASH_BYTES);
    let mark = mark_from(&format!("head1{tail}"), 0);
    assert!(!changed(&mark, &format!("head2{tail}")));
}
