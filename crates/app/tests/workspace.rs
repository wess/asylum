use super::*;

// These tests exercise the pure layout logic with data-only tabs (no gpui
// entities needed).

#[test]
fn starts_with_one_pane_one_tab() {
    let ws = Workspace::new(1);
    assert_eq!(ws.panes.len(), 1);
    assert_eq!(ws.panes[0].tabs.len(), 1);
    assert_eq!(ws.active_key(), Some(TabKey::Tasks));
}

#[test]
fn singleton_open_focuses_existing() {
    let mut ws = Workspace::new(1);
    ws.open(2, TabKind::Search);
    assert_eq!(ws.panes[0].tabs.len(), 2);
    // Re-opening Tasks focuses the existing tab rather than adding one.
    ws.open(3, TabKind::Tasks);
    assert_eq!(ws.panes[0].tabs.len(), 2);
    assert_eq!(ws.active_key(), Some(TabKey::Tasks));
}

#[test]
fn split_adds_a_pane_and_activates_it() {
    let mut ws = Workspace::new(1);
    ws.split(2, TabKind::Search);
    assert_eq!(ws.panes.len(), 2);
    assert_eq!(ws.active_pane, 1);
    assert_eq!(ws.active_key(), Some(TabKey::Search));
}

#[test]
fn close_removes_tab_and_empty_pane() {
    let mut ws = Workspace::new(1);
    ws.split(2, TabKind::Search);
    assert_eq!(ws.panes.len(), 2);
    // Close the only tab in pane 1 → the pane is removed.
    ws.close(1, 0);
    assert_eq!(ws.panes.len(), 1);
    assert_eq!(ws.active_pane, 0);
}

#[test]
fn last_pane_never_removed() {
    let mut ws = Workspace::new(1);
    ws.close(0, 0); // closing the last tab of the last pane
    assert_eq!(ws.panes.len(), 1);
    assert!(ws.panes[0].tabs.is_empty());
}

#[test]
fn activate_selects_pane_and_tab() {
    let mut ws = Workspace::new(1);
    ws.open(2, TabKind::Search);
    ws.open(3, TabKind::Diff);
    ws.activate(0, 0);
    assert_eq!(ws.active_key(), Some(TabKey::Tasks));
}
