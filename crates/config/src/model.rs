//! The typed settings schema.
//!
//! Every field carries a serde default so a partial (or empty) `settings.json`
//! still deserializes into a complete [`Settings`]. `#[serde(default)]` on the
//! struct fills absent keys; per-field defaults set the compiled-in values.

use serde::{Deserialize, Serialize};

/// The full resolved configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Named color theme for the chrome (guise theme selection).
    pub theme: String,
    /// Where per-task worktrees are created, relative to a project root.
    pub worktree_dir: String,
    /// Ids of the agents (from the `agent` registry) enabled by default when a
    /// task is fanned out. Empty means "ask each time".
    pub default_agents: Vec<String>,
    /// How many agents may run concurrently across all tasks. 0 = unlimited.
    pub max_parallel_runs: u32,
    /// Stop an agent after this many minutes. 0 = no timeout.
    pub run_timeout_minutes: u32,
    /// Per-agent overrides keyed by agent id.
    pub agents: std::collections::BTreeMap<String, AgentPrefs>,
    /// Bring-your-own agents: definitions added on top of the built-in catalog.
    pub custom_agents: Vec<CustomAgent>,
    /// Named fan-out presets. Picking a layout when composing a task selects its
    /// set of agents (and optional concurrency) in one gesture, instead of
    /// ticking agents by hand each time.
    pub layouts: Vec<Layout>,
    /// Built-in editor preferences.
    pub editor: EditorPrefs,
    /// Keybindings as `chord=action` strings, layered over the defaults.
    pub keybindings: Vec<String>,
    /// Linear API token. When set, the Integrations surface browses Linear
    /// teams and issues; empty leaves Linear disabled.
    pub linear_token: String,
    /// Mobile companion server preferences.
    pub companion: CompanionPrefs,
    /// Agent control-surface server preferences.
    pub control: ControlPrefs,
    /// Secrets-proxy server preferences (masked outbound API access for agents).
    pub proxy: ProxyPrefs,
    /// Named upstreams the secrets proxy can forward to, each binding a stored
    /// secret to a fixed destination. Secret *values* are never here - each
    /// `secret` names an entry in the encrypted keep (`asylum keep set <name>`),
    /// resolved at request time and injected server-side.
    pub upstreams: Vec<Upstream>,
    /// MCP gateway server preferences. When enabled, Asylum runs one aggregating
    /// MCP server that every agent connects to instead of each configuring N
    /// servers of its own.
    pub mcp: McpPrefs,
    /// The upstream MCP servers the gateway aggregates. Each is exposed under its
    /// own namespace (`<name>__<tool>`) behind the single gateway endpoint.
    pub mcp_servers: Vec<McpServer>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            worktree_dir: ".asylum/worktrees".to_string(),
            default_agents: Vec::new(),
            max_parallel_runs: 4,
            run_timeout_minutes: 60,
            agents: std::collections::BTreeMap::new(),
            custom_agents: Vec::new(),
            layouts: Layout::builtins(),
            editor: EditorPrefs::default(),
            keybindings: Vec::new(),
            linear_token: String::new(),
            companion: CompanionPrefs::default(),
            control: ControlPrefs::default(),
            proxy: ProxyPrefs::default(),
            upstreams: Vec::new(),
            mcp: McpPrefs::default(),
            mcp_servers: Vec::new(),
        }
    }
}

impl Settings {
    /// Look up a fan-out preset by name (case-insensitive).
    pub fn layout(&self, name: &str) -> Option<&Layout> {
        self.layouts
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(name))
    }
}

/// A named fan-out preset. Picking a layout when composing a task fans it out
/// across every listed agent in one gesture, optionally capping how many run at
/// once for that task. A layout is data, not a keybinding: it defines *which*
/// agents race, so the same task shape can be re-run without re-ticking boxes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Layout {
    /// Stable, human-facing name shown in the picker and used by `asylum layout`.
    pub name: String,
    /// One-line description of what the preset is for.
    pub description: String,
    /// Agent ids (from the registry) that each get a run.
    pub agents: Vec<String>,
    /// Max simultaneous runs for a task launched from this layout. 0 defers to
    /// the global `max_parallel_runs`.
    pub concurrency: u32,
}

impl Layout {
    /// The starter presets shipped when the user has not defined their own.
    /// Overridable by setting `layouts` in settings.json.
    pub fn builtins() -> Vec<Layout> {
        vec![
            Layout {
                name: "duel".to_string(),
                description: "Two frontier agents, head to head.".to_string(),
                agents: vec!["claude-code".to_string(), "codex".to_string()],
                concurrency: 0,
            },
            Layout {
                name: "triad".to_string(),
                description: "Three takes on one prompt.".to_string(),
                agents: vec![
                    "claude-code".to_string(),
                    "codex".to_string(),
                    "aider".to_string(),
                ],
                concurrency: 0,
            },
            Layout {
                name: "swarm".to_string(),
                description: "A wide net; three running at a time.".to_string(),
                agents: vec![
                    "claude-code".to_string(),
                    "codex".to_string(),
                    "opencode".to_string(),
                    "gemini".to_string(),
                    "aider".to_string(),
                ],
                concurrency: 3,
            },
        ]
    }
}

/// Mobile companion HTTP server preferences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CompanionPrefs {
    /// Whether the companion server runs at all.
    pub enabled: bool,
    /// Address to bind. Localhost by default; set to `0.0.0.0:8787` to reach it
    /// from a phone on the LAN. A non-loopback bind requires a token - the app
    /// refuses to start the server on a LAN/wildcard address without one.
    pub bind: String,
    /// Bearer token required on API requests. Empty is allowed only for a
    /// loopback bind (localhost-only, no auth); a non-loopback bind without a
    /// token is refused at startup. When set, it is required as
    /// `Authorization: Bearer <token>` on every `/api/*` request.
    pub token: String,
}

impl Default for CompanionPrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            bind: "127.0.0.1:8787".to_string(),
            token: String::new(),
        }
    }
}

/// Agent control-surface server preferences. The control server lets a running
/// agent orchestrate the fleet from inside its worktree; because it can spawn
/// runs and read transcripts, it is loopback-only and always authenticated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ControlPrefs {
    /// Whether the control server runs at all. When off, agents cannot
    /// orchestrate the ADE (the `asylum control` commands report "not inside a
    /// worktree").
    pub enabled: bool,
    /// Address to bind. Loopback only - a non-loopback bind is refused at
    /// startup. An agent reaches it at `127.0.0.1:<port>`.
    pub bind: String,
    /// Bearer token required on control requests. When empty (the default) the
    /// app provisions a strong per-session token, kept in memory only and never
    /// written back to settings. Either way the token is injected into each
    /// managed agent as `ASYLUM_CONTROL_TOKEN`; localhost is not treated as an
    /// authentication boundary.
    pub token: String,
}

impl Default for ControlPrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            bind: "127.0.0.1:8788".to_string(),
            token: String::new(),
        }
    }
}

/// Secrets-proxy server preferences. The proxy lets a running agent make
/// outbound API calls through named [`Upstream`]s without ever seeing the
/// credentials: the agent hits `http://127.0.0.1:<port>/<upstream>/<path>`, the
/// proxy injects the real secret server-side and forwards only to that
/// upstream's host. Loopback-only and always authenticated, like the control
/// surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProxyPrefs {
    /// Whether the secrets proxy runs at all. Off by default - it only does
    /// something once you define `upstreams`.
    pub enabled: bool,
    /// Address to bind. Loopback only - a non-loopback bind is refused at
    /// startup. Agents reach it at `127.0.0.1:<port>`.
    pub bind: String,
}

impl Default for ProxyPrefs {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: "127.0.0.1:8789".to_string(),
        }
    }
}

/// A named upstream the secrets proxy can forward to. It binds a stored secret
/// to a fixed destination and describes how to inject the secret - so an agent
/// can *use* the credential but never learn it, and the secret only ever travels
/// to `base_url`'s host (no exfiltration).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Upstream {
    /// The name the agent addresses (`/<name>/...`). Lowercase slug.
    pub name: String,
    /// The upstream base URL, e.g. `https://api.openai.com`. Requests are
    /// forwarded to `base_url` + the path after `/<name>`. Only this host ever
    /// receives the secret.
    pub base_url: String,
    /// Which secret to inject: the value is resolved from the encrypted keep
    /// (never from this file), scoped to the calling agent's project.
    pub secret: String,
    /// The header the secret is injected into (default `Authorization`).
    pub header: String,
    /// How the header value is formatted; `{secret}` is replaced with the
    /// resolved secret value (default `Bearer {secret}`).
    pub format: String,
    /// The project this upstream belongs to (a project id), or `0` for a global
    /// upstream available to every project. A project-scoped upstream overrides a
    /// global one of the same name for that project.
    pub project: i64,
}

/// MCP gateway preferences. The gateway is one MCP server Asylum runs on
/// loopback that every managed agent connects to; it aggregates the configured
/// [`McpServer`]s so an agent sees one connection carrying every service's tools
/// (each namespaced `<service>__<tool>`) instead of configuring N servers apiece.
/// Loopback-only and token-authenticated, like the control surface and proxy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct McpPrefs {
    /// Whether the gateway runs at all. Off by default - it only does something
    /// once you define `mcp_servers`.
    pub enabled: bool,
    /// Address to bind. Loopback only - a non-loopback bind is refused at
    /// startup. Agents reach it at `http://127.0.0.1:<port>/mcp`.
    pub bind: String,
    /// How the aggregated tools are exposed to the agent:
    /// - `"direct"` (default): every upstream tool is listed, namespaced. Simple,
    ///   but the agent's context carries every tool definition.
    /// - `"search"`: the gateway advertises just two meta-tools
    ///   (`asylum_find_tool` / `asylum_call_tool`); the agent searches for a tool
    ///   and invokes it by name, so tool definitions load on demand. Keeps a wide
    ///   fleet's context small.
    pub expose: String,
}

impl Default for McpPrefs {
    fn default() -> Self {
        Self {
            enabled: false,
            bind: "127.0.0.1:8790".to_string(),
            expose: "direct".to_string(),
        }
    }
}

/// One upstream MCP server the gateway aggregates. It is exposed to agents under
/// `name` as a namespace: the upstream's `list_tools` become `<name>__<tool>`,
/// its resource URIs are prefixed `<name>__`, and a call to a namespaced tool is
/// routed back to this server. The upstream is either a local process
/// (`transport = "stdio"`, launched from `command`/`args`) or a remote HTTP MCP
/// server (`transport = "http"`, reached at `url`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct McpServer {
    /// The namespace slug agents see (`<name>__<tool>`). Lowercase; no `__`.
    pub name: String,
    /// Transport: `"stdio"` (default, a local child process) or `"http"`.
    pub transport: String,
    /// stdio: the program to launch (looked up on PATH).
    pub command: String,
    /// stdio: arguments passed to `command`.
    pub args: Vec<String>,
    /// http: the MCP endpoint URL (e.g. `https://mcp.example.com/mcp`).
    pub url: String,
    /// Extra environment for a stdio child. A value of the form `{secret:NAME}`
    /// is resolved from the encrypted keep at spawn time (scoped to the run's
    /// project), so an API key never sits in this file; any other value is
    /// passed through literally.
    pub env: std::collections::BTreeMap<String, String>,
    /// http: the secret injected as an auth header, resolved from the keep by
    /// name (never stored here). Empty means the upstream needs no auth.
    pub secret: String,
    /// http: the header the secret is injected into (default `Authorization`).
    pub header: String,
    /// http: how the header value is formatted; `{secret}` is replaced with the
    /// resolved secret (default `Bearer {secret}`).
    pub format: String,
    /// Only expose these tool names (by their *upstream* name, before
    /// namespacing). Empty means "all".
    pub allow: Vec<String>,
    /// Hide these tool names (by their upstream name). Applied after `allow`.
    pub deny: Vec<String>,
    /// The project this server belongs to (a project id), or `0` for a global
    /// server visible to every project. A project-scoped server overrides a
    /// global one of the same name for that project.
    pub project: i64,
    /// Whether this server is aggregated at all (lets one be disabled without
    /// deleting its config). Defaults to `true` for a listed server.
    pub enabled: bool,
}

impl Default for McpServer {
    fn default() -> Self {
        Self {
            name: String::new(),
            transport: "stdio".to_string(),
            command: String::new(),
            args: Vec::new(),
            url: String::new(),
            env: std::collections::BTreeMap::new(),
            secret: String::new(),
            header: String::new(),
            format: String::new(),
            allow: Vec::new(),
            deny: Vec::new(),
            project: 0,
            enabled: true,
        }
    }
}

/// Per-agent user overrides.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentPrefs {
    /// Override the launch program (e.g. a wrapper script).
    pub program: Option<String>,
    /// Extra arguments appended to the agent's command line.
    pub extra_args: Vec<String>,
    /// Force-enable or disable this agent regardless of `default_agents`.
    pub enabled: Option<bool>,
}

/// A user-defined agent added to the catalog ("bring your own agent").
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CustomAgent {
    /// Stable id (used in fan-out, branch names, and the store).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Single-glyph icon.
    pub icon: String,
    /// Program to launch (looked up on PATH).
    pub program: String,
    /// Argument template; `{prompt}` is substituted under `arg` delivery.
    pub args: Vec<String>,
    /// How the prompt is delivered: `"arg"` (default) or `"stdin"`.
    pub delivery: String,
}

/// Built-in code-editor preferences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EditorPrefs {
    pub font_family: String,
    pub font_size: f32,
    pub tab_width: u32,
    pub autosave: bool,
}

impl Default for EditorPrefs {
    fn default() -> Self {
        Self {
            font_family: "monospace".to_string(),
            font_size: 13.0,
            tab_width: 4,
            autosave: true,
        }
    }
}

#[cfg(test)]
#[path = "../tests/model.rs"]
mod tests;
