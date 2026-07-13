use super::*;

#[test]
fn snapshot_commands_per_os() {
    let (p, a) = snapshot_command("macos", "out.png");
    assert_eq!(p, "screencapture");
    assert!(a.contains(&"out.png".to_string()));

    let (p, a) = snapshot_command("linux", "out.png");
    assert_eq!(p, "scrot");
    assert!(a.contains(&"out.png".to_string()));
}

#[test]
fn click_commands_per_os() {
    let (p, a) = click_command("macos", 100, 200);
    assert_eq!(p, "cliclick");
    assert_eq!(a, vec!["c:100,200"]);

    let (p, a) = click_command("linux", 100, 200);
    assert_eq!(p, "xdotool");
    assert!(a.contains(&"mousemove".to_string()));
    assert!(a.contains(&"100".to_string()));
    assert!(a.contains(&"200".to_string()));
}

#[test]
fn fill_commands_per_os() {
    let (p, a) = fill_command("macos", "hello");
    assert_eq!(p, "osascript");
    assert!(a[1].contains("keystroke \"hello\""));

    let (p, a) = fill_command("linux", "hello");
    assert_eq!(p, "xdotool");
    assert_eq!(a, vec!["type", "hello"]);
}

#[test]
fn fill_escapes_quotes_on_macos() {
    let (_, a) = fill_command("macos", "say \"hi\"");
    assert!(a[1].contains("\\\"hi\\\""));
}
