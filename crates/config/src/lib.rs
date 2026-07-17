//! Layered settings for the Agent Development Environment.
//!
//! Configuration is compiled-in defaults overridden by the user's
//! `settings.json` (JSON with `//` and `/* */` comments). A bad value never
//! aborts the load: it is dropped, replaced by the default, and recorded as a
//! [`Diagnostic`] the app can surface - the app always gets a usable
//! [`Settings`], with warnings on the side.
//!
//! Submodules:
//! - [`model`] - the typed [`Settings`] schema (serde defaults).
//! - [`jsonc`] - strip comments so `serde_json` can parse the file.
//! - [`load`] - path resolution and file loading.
//! - [`edit`] - comment-preserving writes back to settings.json.
//! - [`watch`] - live reload: poll the file's mtime, fire on change.
//! - [`bind`] - refuse unsafe (unauthenticated non-loopback) server binds.
//! - [`token`] - generate the control surface's per-session credential.

pub mod bind;
pub mod edit;
mod jsonc;
pub mod keys;
pub mod load;
pub mod model;
pub mod project;
pub mod token;
pub mod watch;

pub use keys::{Keymap, DEFAULTS as KEY_DEFAULTS};
pub use load::{default_path, load, load_str, Loaded};
pub use model::{
    AgentPrefs, CompanionPrefs, ControlPrefs, CustomAgent, EditorPrefs, Layout, McpPrefs,
    McpServer, ProxyPrefs, Settings, Upstream,
};
pub use project::{load_project, parse_project, ProjectConfig, PROJECT_FILE};
pub use watch::{watch, WatchHandle};

/// A non-fatal problem found while loading settings: a JSON parse failure or a
/// value that could not be understood. The load continues with defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Dotted path to the offending key (empty for a whole-file parse error).
    pub key: String,
    pub message: String,
}

impl Diagnostic {
    pub(crate) fn new(key: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            message: message.into(),
        }
    }
}
