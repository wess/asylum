//! Manifest-based plugins for the Agent Development Environment.
//!
//! A plugin is a directory containing a `plugin.toml` manifest. Modeled on
//! a manifest-based extension model, a manifest contributes:
//!
//! - `[[command]]` - palette actions with optional default keybindings.
//! - `[panel]` - a side-drawer panel rendered from the plugin's runtime.
//! - `[webview]` - a native web surface (panel / tab / window).
//! - `[[trigger]]` - hooks that fire an action on an ADE event
//!   (`run_finished`, `worktree_created`, …; see [`TRIGGER_EVENTS`]).
//! - `[[tool]]` - tools exposed to the coding agents themselves.
//!
//! Of those five, only `[[command]]` reaches the user today: the host (`app`)
//! invokes commands through the runtime. `[panel]`, `[webview]`, `[[trigger]]`,
//! and `[[tool]]` are parsed and validated here, but the host does not yet
//! render the surfaces, dispatch triggers on ADE events, or expose tools to the
//! agents. They are manifest-level today - a plugin can declare them, and the
//! vocabulary is stable, but nothing fires them.
//!
//! Plugins declare `capabilities` (see [`CAPABILITIES`]) - advisory under the
//! process runtime (`pluginrt`), and the vocabulary the WASM runtime enforces.
//! This crate is pure parsing + validation; the host drives the runtime.
//!
//! Submodules:
//! - [`model`] - the parsed manifest types and fixed vocabularies.
//! - [`parse`] - TOML → [`Plugin`], with friendly [`Diagnostic`]s.
//! - [`load`] - discover and load plugins from a directory.

pub mod install;
pub mod load;
pub mod model;
pub mod parse;

pub use install::{clone_command, discover_command, fetch, Source, TOPIC};
pub use load::{default_dir, load_dir, Installed};
pub use model::{
    Command, CommandMode, Diagnostic, Panel, Placement, Plugin, Runtime, RuntimeKind, Tool,
    ToolParam, Trigger, TriggerAction, Webview, WebviewSource, CAPABILITIES, TRIGGER_EVENTS,
};
pub use parse::parse;

/// The manifest filename inside a plugin directory.
pub const MANIFEST: &str = "plugin.toml";

/// Stable action id for a contributed command: `<plugin-id>/<command-id>`.
pub fn action_id(plugin: &str, command: &str) -> String {
    format!("{plugin}/{command}")
}

/// Find a command by the action id returned from [`action_id`].
pub fn command<'a>(plugins: &'a [Plugin], id: &str) -> Option<(&'a Plugin, &'a Command)> {
    let (plugin_id, command_id) = id.split_once('/')?;
    let plugin = plugins.iter().find(|p| p.id == plugin_id)?;
    let command = plugin.commands.iter().find(|c| c.id == command_id)?;
    Some((plugin, command))
}
