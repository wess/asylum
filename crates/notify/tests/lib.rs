use super::*;

#[test]
fn macos_builds_osascript() {
    let n = Notification::new("Codex done", "3 files changed");
    let (program, args) = command_for("macos", &n);
    assert_eq!(program, "osascript");
    assert_eq!(args[0], "-e");
    assert!(args[1].contains("display notification \"3 files changed\""));
    assert!(args[1].contains("with title \"Codex done\""));
}

#[test]
fn macos_includes_subtitle() {
    let n = Notification::new("t", "b").subtitle("acme-web");
    let (_, args) = command_for("macos", &n);
    assert!(args[1].contains("subtitle \"acme-web\""));
}

#[test]
fn linux_builds_notify_send() {
    let n = Notification::new("Title", "Body");
    let (program, args) = command_for("linux", &n);
    assert_eq!(program, "notify-send");
    assert_eq!(args, vec!["Title", "Body"]);
}

#[test]
fn applescript_escapes_quotes() {
    let n = Notification::new("She said \"hi\"", "path\\to\\thing");
    let (_, args) = command_for("macos", &n);
    // Embedded quotes and backslashes are escaped so the script stays valid.
    assert!(args[1].contains("\\\"hi\\\""));
    assert!(args[1].contains("path\\\\to\\\\thing"));
}
