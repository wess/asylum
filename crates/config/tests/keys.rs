use super::*;

#[test]
fn defaults_bind_core_actions() {
    let km = Keymap::defaults();
    assert_eq!(km.action("cmd-k"), Some("command_palette"));
    assert_eq!(km.action("cmd-enter"), Some("run_fanout"));
    assert!(km.action("cmd-unbound").is_none());
    assert!(km.len() >= 10);
}

#[test]
fn user_binding_overrides_default() {
    let km = Keymap::from_settings(&["cmd-k=quick_open".to_string()]);
    assert_eq!(km.action("cmd-k"), Some("quick_open"));
}

#[test]
fn user_can_add_new_binding() {
    let km = Keymap::from_settings(&["ctrl-g=goto_line".to_string()]);
    assert_eq!(km.action("ctrl-g"), Some("goto_line"));
    // Defaults remain.
    assert_eq!(km.action("cmd-p"), Some("quick_open"));
}

#[test]
fn empty_action_unbinds() {
    let km = Keymap::from_settings(&["cmd-k=".to_string()]);
    assert!(km.action("cmd-k").is_none());
}

#[test]
fn parse_binding_normalizes_case() {
    assert_eq!(
        parse_binding("CMD-K=command_palette"),
        Some(("cmd-k".to_string(), "command_palette".to_string()))
    );
    assert_eq!(parse_binding("no-equals"), None);
    assert_eq!(parse_binding("=orphan"), None);
}
