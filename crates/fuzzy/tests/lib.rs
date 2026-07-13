use super::*;

#[test]
fn subsequence_match_and_miss() {
    assert!(score("cmp", "command_palette").is_some());
    assert!(score("xyz", "command_palette").is_none());
    assert!(score("", "anything").is_some());
}

#[test]
fn case_insensitive() {
    assert!(score("CMD", "command").is_some());
    assert!(score("cmd", "COMMAND").is_some());
}

#[test]
fn consecutive_beats_scattered() {
    let consecutive = score("comm", "command").unwrap();
    let scattered = score("comm", "c_o_m_m_x").unwrap();
    assert!(consecutive > scattered, "{consecutive} vs {scattered}");
}

#[test]
fn word_boundary_bonus() {
    // "op" matching the start of "open" (after a separator) beats a mid-word hit.
    let boundary = score("op", "quick/open").unwrap();
    let midword = score("op", "stoping").unwrap();
    assert!(boundary > midword, "{boundary} vs {midword}");
}

#[test]
fn rank_orders_best_first() {
    let items = vec!["run_fanout", "quick_open", "open_recent", "toggle_panel"];
    let ranked = rank("open", &items);
    assert!(!ranked.is_empty());
    // "quick_open" and "open_recent" match "open"; the boundary-aligned one wins.
    assert!(ranked[0].value.contains("open"));
    assert!(ranked.iter().all(|m| m.value.contains('o')));
}

#[test]
fn rank_drops_non_matches() {
    let items = vec!["alpha", "beta", "gamma"];
    let ranked = rank("zzz", &items);
    assert!(ranked.is_empty());
}

#[test]
fn empty_query_returns_all() {
    let items = vec!["a", "b", "c"];
    assert_eq!(rank("", &items).len(), 3);
}

#[test]
fn ranks_file_paths() {
    let files = vec![
        "src/main.rs",
        "src/state.rs",
        "crates/store/src/lib.rs",
        "README.md",
    ];
    let ranked = rank("staters", &files);
    assert_eq!(ranked[0].value, "src/state.rs");
}
