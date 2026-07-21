use super::*;

// The rail's progressive disclosure is plain data: a primary set always shown
// and a "More" set revealed on demand. These tests pin the partition and the
// hidden-surface reveal rule.

#[test]
fn rail_partition_covers_every_surface_exactly_once() {
    // Exhaustive on purpose: adding a `View` variant fails to compile here
    // until it is placed on the rail or excluded as chrome.
    fn on_rail(view: View) -> bool {
        match view {
            View::Tasks
            | View::Diff
            | View::Search
            | View::Notes
            | View::Integrations
            | View::Terminal
            | View::Editor
            | View::Preview
            | View::Browser
            | View::Plugins
            | View::Accounts
            | View::Notifications => true,
            View::Settings => false,
        }
    }
    let rail: Vec<View> = View::PRIMARY.iter().chain(View::MORE).copied().collect();
    assert_eq!(rail.len(), 12, "primary + more carry every rail surface");
    for (index, view) in rail.iter().enumerate() {
        assert!(on_rail(*view), "{view:?} is chrome, not a rail entry");
        assert!(!rail[index + 1..].contains(view), "{view:?} listed twice");
    }
}

#[test]
fn primary_is_the_core_loop() {
    assert_eq!(View::PRIMARY, &[View::Tasks, View::Diff, View::Search]);
}

#[test]
fn hidden_surfaces_stay_off_the_rail_until_revealed() {
    assert_eq!(more_rail(None, false), Vec::<View>::new());
    assert_eq!(more_rail(Some(View::Tasks), false), Vec::<View>::new());
    assert_eq!(more_rail(Some(View::Tasks), true), View::MORE.to_vec());
}

#[test]
fn an_open_hidden_surface_is_revealed_while_active() {
    // Opening Notes from the palette with "More" collapsed still shows its
    // rail entry, so the active tab is never unrepresented.
    assert_eq!(more_rail(Some(View::Notes), false), vec![View::Notes]);
    // Settings is chrome with its own affordance, never a "More" entry.
    assert_eq!(more_rail(Some(View::Settings), false), Vec::<View>::new());
}
