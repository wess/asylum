//! The parsed `plugin.toml` data model and the fixed vocabularies. No parsing
//! logic here — see [`parse`](crate::parse).

use std::path::PathBuf;

/// A non-fatal problem found while loading a manifest. Bad plugins are skipped
/// with a diagnostic rather than aborting discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub message: String,
}

/// A fully parsed plugin manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    /// The plugin's directory (where `plugin.toml` lives).
    pub path: PathBuf,
    pub commands: Vec<Command>,
    /// An executable the app talks to over JSON stdio to render panels and
    /// handle actions/tools. Present makes this a runtime plugin.
    pub runtime: Option<Runtime>,
    pub panel: Option<Panel>,
    pub webview: Option<Webview>,
    pub triggers: Vec<Trigger>,
    /// Tools this plugin exposes to the coding agents (via the app's MCP bridge).
    pub tools: Vec<Tool>,
    /// Declared capabilities (from [`CAPABILITIES`]).
    pub capabilities: Vec<String>,
}

/// The capabilities a plugin may declare. Advisory under the process runtime;
/// the gate list the WASM runtime will enforce.
pub const CAPABILITIES: &[&str] = &[
    "git",        // read/modify worktrees and branches
    "agents",     // start/inspect agent runs
    "store",      // read tasks and runs
    "network",    // make network requests
    "filesystem", // read or write files
    "clipboard",  // read or write the clipboard
    "notify",     // post desktop notifications
];

/// The ADE events a `[[trigger]]` may hook.
pub const TRIGGER_EVENTS: &[&str] = &[
    "task_created",
    "run_started",
    "run_finished",
    "run_failed",
    "worktree_created",
    "worktree_removed",
    "diff_ready",
    "task_merged",
];

/// `[runtime]` — how to launch the plugin's function host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runtime {
    pub kind: RuntimeKind,
    /// For a `process` runtime: the command to spawn (split on whitespace).
    pub command: String,
    /// For a `wasm` runtime: the `.wasm` module path, relative to the plugin.
    pub wasm: Option<String>,
    /// A `process` runtime that is a long-lived stdio server (kept warm) rather
    /// than spawned per event.
    pub persistent: bool,
}

/// The kind of `[runtime]` host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeKind {
    /// A subprocess spoken to over JSON on stdin/stdout. Full user privileges.
    #[default]
    Process,
    /// A WebAssembly module run in-process, sandboxed to its declared
    /// capabilities. Declaration is supported; execution is planned — see
    /// docs/plugins.md.
    Wasm,
}

impl RuntimeKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "process" | "" => Some(Self::Process),
            "wasm" | "wasm32" | "webassembly" => Some(Self::Wasm),
            _ => None,
        }
    }
}

/// `[panel]` — a contributed side-drawer panel rendered from runtime responses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    pub id: String,
    pub title: String,
    /// Single-glyph activity-bar icon.
    pub icon: String,
}

/// `[webview]` — a native web surface a plugin contributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Webview {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub placement: Placement,
    pub source: WebviewSource,
}

/// Where a `[webview]` surface is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placement {
    #[default]
    Panel,
    Tab,
    Window,
}

impl Placement {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "panel" => Some(Self::Panel),
            "tab" => Some(Self::Tab),
            "window" => Some(Self::Window),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Tab => "tab",
            Self::Window => "window",
        }
    }
}

/// Where a `[webview]` loads its content from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebviewSource {
    /// A remote or absolute URL, loaded as-is.
    Url(String),
    /// A path relative to the plugin directory, served over the internal origin.
    Entry(String),
    /// A host-managed sidecar: the host runs this command as a local server and
    /// loads the page from its `http` origin.
    Service(String),
}

/// `[[tool]]` — a tool a plugin exposes to the coding agents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    /// Stable id; the exposed tool name is `<plugin-id>_<id>`.
    pub id: String,
    pub description: String,
    pub params: Vec<ToolParam>,
}

/// One argument of a `[[tool]]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolParam {
    pub name: String,
    /// JSON Schema type: `string` | `number` | `integer` | `boolean`.
    pub kind: String,
    pub description: String,
    pub required: bool,
}

/// `[[trigger]]` — run an action when an ADE event fires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// The event to hook; one of [`TRIGGER_EVENTS`].
    pub on: String,
    /// Optional event-specific filter (interpreted by the host).
    pub when: Option<String>,
    pub action: TriggerAction,
}

/// What a [`Trigger`] does when it fires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerAction {
    /// Post a desktop notification with this body.
    Notify { text: String },
    /// Call the plugin's `[runtime]` with the event payload (method name).
    Invoke { method: String },
}

/// A palette command a plugin contributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub id: String,
    pub title: String,
    /// The runtime method invoked when the command runs.
    pub run: String,
    pub mode: CommandMode,
    pub keybind: Option<String>,
}

/// How a contributed command surfaces its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandMode {
    /// Invoke the runtime and show its result inline (default).
    #[default]
    Invoke,
    /// Open the plugin's panel.
    Panel,
    /// Open the plugin's webview.
    Webview,
}

impl CommandMode {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "invoke" | "" => Some(Self::Invoke),
            "panel" => Some(Self::Panel),
            "webview" => Some(Self::Webview),
            _ => None,
        }
    }
}
