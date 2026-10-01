//! On macOS the computer's shell runs under `sandbox-exec` with a profile
//! that allows reads broadly but writes only to the workspace, temp dirs, and
//! tool caches — the local stand-in for the cloud VM's isolation. Commands on
//! the user's own machine (local execution) skip this.

use std::path::Path;

/// Outbound network for a sandboxed command.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Net {
  #[default]
  Open,
  /// Only through the egress proxy on this port (and localhost).
  Proxy(u16),
  /// Localhost only.
  Offline,
}

/// Network rules appended to a profile.
pub fn net_rules(net: Net) -> &'static str {
  match net {
    Net::Open => "",
    Net::Proxy(_) | Net::Offline => {
      "(deny network-outbound)\n(allow network-outbound (remote ip \"localhost:*\"))\n(allow network-outbound (remote unix-socket))\n"
    }
  }
}

/// A profile that only limits the network (for commands on the user's own
/// Mac when the admin turns off local egress).
pub fn net_only(net: Net) -> String {
  format!("(version 1)\n(allow default)\n{}", net_rules(net))
}

pub fn profile_with(workspace: &Path, net: Net) -> String {
  format!("{}{}", profile(workspace), net_rules(net))
}

pub fn profile(workspace: &Path) -> String {
  let ws = workspace.display().to_string().replace('"', "");
  let home = std::env::var("HOME").unwrap_or_default().replace('"', "");
  format!(
    r#"(version 1)
(allow default)
(deny file-write*)
(allow file-write*
  (subpath "{ws}")
  (subpath "/private/tmp")
  (subpath "/private/var/folders")
  (subpath "/dev")
  (subpath "{home}/.cache")
  (subpath "{home}/.npm")
  (subpath "{home}/.cargo/registry")
  (subpath "{home}/Library/Caches"))
"#
  )
}

pub fn available() -> bool {
  cfg!(target_os = "macos") && Path::new("/usr/bin/sandbox-exec").exists()
}
