//! Computer Use: drive the desktop - screenshot, click, and type - so an agent
//! (or a script) can operate GUI apps. Each function builds the platform command
//! (`screencapture`/`cliclick`/`osascript` on macOS, `scrot`/`xdotool` on Linux)
//! and is pure, so the builders are unit-tested on any host; execution is a thin
//! wrapper.

use std::process::Command;

/// Build the (program, args) to capture the full screen to `out` for `os`.
pub fn snapshot_command(os: &str, out: &str) -> (String, Vec<String>) {
    if os == "macos" {
        ("screencapture".into(), vec!["-x".into(), out.into()])
    } else {
        ("scrot".into(), vec!["--overwrite".into(), out.into()])
    }
}

/// Build the (program, args) to click at (`x`, `y`) for `os`.
pub fn click_command(os: &str, x: i32, y: i32) -> (String, Vec<String>) {
    if os == "macos" {
        ("cliclick".into(), vec![format!("c:{x},{y}")])
    } else {
        (
            "xdotool".into(),
            vec![
                "mousemove".into(),
                x.to_string(),
                y.to_string(),
                "click".into(),
                "1".into(),
            ],
        )
    }
}

/// Build the (program, args) to type `text` for `os`.
pub fn fill_command(os: &str, text: &str) -> (String, Vec<String>) {
    if os == "macos" {
        let script = format!(
            "tell application \"System Events\" to keystroke {}",
            applescript_quote(text)
        );
        ("osascript".into(), vec!["-e".into(), script])
    } else {
        ("xdotool".into(), vec!["type".into(), text.into()])
    }
}

/// Run a built command, mapping a launch failure to a friendly message.
pub fn run(program: &str, args: &[String]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|e| format!("{program} not available: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}"))
    }
}

fn applescript_quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
#[path = "../tests/computer.rs"]
mod tests;
