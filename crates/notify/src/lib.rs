//! Desktop notifications.
//!
//! When an agent finishes, needs attention, or a check fails, the ADE posts a
//! desktop notification (and the mobile companion mirrors it). This crate builds
//! the platform command — `osascript` on macOS, `notify-send` on Linux — and
//! sends it. The command builder is pure and tested for both platforms; only
//! [`send`] touches the system.

use std::process::Command;

/// A notification to post.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub title: String,
    pub body: String,
    /// macOS subtitle (ignored on Linux).
    pub subtitle: Option<String>,
}

impl Notification {
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Notification {
            title: title.into(),
            body: body.into(),
            subtitle: None,
        }
    }

    pub fn subtitle(mut self, s: impl Into<String>) -> Self {
        self.subtitle = Some(s.into());
        self
    }
}

/// Build the (program, args) for a target OS (`"macos"` or anything else →
/// Linux `notify-send`). Split out from [`send`] so both branches are testable
/// on any host.
pub fn command_for(os: &str, n: &Notification) -> (String, Vec<String>) {
    if os == "macos" {
        let mut script = format!(
            "display notification {} with title {}",
            applescript_quote(&n.body),
            applescript_quote(&n.title)
        );
        if let Some(sub) = &n.subtitle {
            script.push_str(&format!(" subtitle {}", applescript_quote(sub)));
        }
        ("osascript".to_string(), vec!["-e".to_string(), script])
    } else {
        (
            "notify-send".to_string(),
            vec![n.title.clone(), n.body.clone()],
        )
    }
}

/// Post `n` on the current platform. Returns `Ok(())` on a successful launch;
/// a missing notifier is surfaced as an error the caller can ignore.
pub fn send(n: &Notification) -> std::io::Result<()> {
    let (program, args) = command_for(std::env::consts::OS, n);
    let status = Command::new(program).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("notifier exited non-zero"))
    }
}

/// Quote and escape a string as an AppleScript string literal.
fn applescript_quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
