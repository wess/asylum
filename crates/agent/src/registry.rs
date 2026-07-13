//! The catalog of coding agents and how each is launched.
//!
//! Two shapes: [`AgentDef`] is a `'static` built-in (the compiled catalog), and
//! [`Agent`] is the owned, resolved form used everywhere else — it can also come
//! from a user's `config::CustomAgent` ("bring your own agent"). Argument
//! templates use the `{prompt}` token, substituted at [`command`](crate::command)
//! build time. Definitions are starting points, overridable via
//! `config::AgentPrefs`, since agent CLIs change often.

use config::CustomAgent;

/// How a prompt reaches an agent process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// Substituted into the argument template wherever `{prompt}` appears (or
    /// appended as a final argument if the template has no token).
    Arg,
    /// Written to the process's stdin; the argument template is used as-is.
    Stdin,
}

impl Delivery {
    /// Parse a config token; anything but `stdin` is [`Delivery::Arg`].
    pub fn parse(s: &str) -> Self {
        match s {
            "stdin" => Delivery::Stdin,
            _ => Delivery::Arg,
        }
    }
}

/// A `'static` built-in agent definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentDef {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub program: &'static str,
    pub args: &'static [&'static str],
    pub delivery: Delivery,
}

impl AgentDef {
    /// Convert a built-in into the owned [`Agent`] form.
    pub fn to_agent(&self) -> Agent {
        Agent {
            id: self.id.to_string(),
            name: self.name.to_string(),
            icon: self.icon.to_string(),
            program: self.program.to_string(),
            args: self.args.iter().map(|s| s.to_string()).collect(),
            delivery: self.delivery,
            builtin: true,
        }
    }
}

/// An owned, resolved agent — from a built-in or a user's custom definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub program: String,
    pub args: Vec<String>,
    pub delivery: Delivery,
    /// True when this came from the built-in catalog.
    pub builtin: bool,
}

impl Agent {
    /// Build an [`Agent`] from a user's [`CustomAgent`] config entry.
    pub fn from_custom(c: &CustomAgent) -> Self {
        Agent {
            id: c.id.clone(),
            name: if c.name.is_empty() { c.id.clone() } else { c.name.clone() },
            icon: if c.icon.is_empty() { "•".to_string() } else { c.icon.clone() },
            program: c.program.clone(),
            args: c.args.clone(),
            delivery: Delivery::parse(&c.delivery),
            builtin: false,
        }
    }
}

/// A prompt-as-argument template shared by most agents.
const ARG_PROMPT: &[&str] = &["{prompt}"];

/// The built-in agent catalog. Order is display order.
pub const BUILTINS: &[AgentDef] = &[
    def("claude-code", "Claude Code", "✳", "claude", &["-p", "{prompt}"], Delivery::Arg),
    def("codex", "Codex", "◆", "codex", &["exec", "{prompt}"], Delivery::Arg),
    def("opencode", "OpenCode", "◇", "opencode", &["run", "{prompt}"], Delivery::Arg),
    def("gemini", "Gemini CLI", "✧", "gemini", &["-p", "{prompt}"], Delivery::Arg),
    def("grok", "Grok", "𝕏", "grok", ARG_PROMPT, Delivery::Arg),
    def("cursor-agent", "Cursor Agent", "❯", "cursor-agent", ARG_PROMPT, Delivery::Arg),
    def("copilot", "GitHub Copilot", "", "copilot", &["-p", "{prompt}"], Delivery::Arg),
    def("aider", "Aider", "▲", "aider", &["--yes", "--message", "{prompt}"], Delivery::Arg),
    def("continue", "Continue", "▶", "cn", ARG_PROMPT, Delivery::Arg),
    def("cline", "Cline", "◈", "cline", ARG_PROMPT, Delivery::Arg),
    def("goose", "Goose", "🪿", "goose", &["run", "-t", "{prompt}"], Delivery::Arg),
    def("amp", "Amp", "⚡", "amp", ARG_PROMPT, Delivery::Stdin),
    def("droid", "Droid", "🤖", "droid", ARG_PROMPT, Delivery::Arg),
    def("qwen-code", "Qwen Code", "◐", "qwen", &["-p", "{prompt}"], Delivery::Arg),
    def("kimi", "Kimi", "◑", "kimi", ARG_PROMPT, Delivery::Arg),
    def("kilocode", "Kilocode", "◒", "kilocode", ARG_PROMPT, Delivery::Arg),
    def("kiro", "Kiro", "◓", "kiro", ARG_PROMPT, Delivery::Arg),
    def("codebuff", "Codebuff", "◔", "codebuff", ARG_PROMPT, Delivery::Arg),
    def("mistral-vibe", "Mistral Vibe", "◕", "vibe", ARG_PROMPT, Delivery::Arg),
    def("pi", "Pi", "π", "pi", ARG_PROMPT, Delivery::Arg),
    def("oh-my-pi", "oh-my-pi", "π", "ohmypi", ARG_PROMPT, Delivery::Arg),
    def("hermes", "Hermes Agent", "☿", "hermes", ARG_PROMPT, Delivery::Arg),
    def("devin", "Devin", "◗", "devin", ARG_PROMPT, Delivery::Arg),
    def("auggie", "Auggie", "◘", "auggie", ARG_PROMPT, Delivery::Arg),
    def("autohand", "Autohand Code", "◙", "autohand", ARG_PROMPT, Delivery::Arg),
    def("charm", "Charm", "✦", "crush", ARG_PROMPT, Delivery::Arg),
    def("command-code", "Command Code", "⌘", "command-code", ARG_PROMPT, Delivery::Arg),
    def("rovo-dev", "Rovo Dev", "◚", "rovodev", ARG_PROMPT, Delivery::Arg),
    def("mimo-code", "MiMo Code", "◛", "mimo", ARG_PROMPT, Delivery::Arg),
    def("openclaude", "OpenClaude", "◜", "openclaude", ARG_PROMPT, Delivery::Arg),
    def("antigravity", "Antigravity", "◝", "antigravity", ARG_PROMPT, Delivery::Arg),
];

/// Const-fn helper so the catalog table stays compact and readable.
const fn def(
    id: &'static str,
    name: &'static str,
    icon: &'static str,
    program: &'static str,
    args: &'static [&'static str],
    delivery: Delivery,
) -> AgentDef {
    AgentDef {
        id,
        name,
        icon,
        program,
        args,
        delivery,
    }
}

/// All built-in agent definitions.
pub fn builtins() -> &'static [AgentDef] {
    BUILTINS
}

/// Look up a built-in agent by id.
pub fn find(id: &str) -> Option<&'static AgentDef> {
    BUILTINS.iter().find(|a| a.id == id)
}

/// The resolved catalog: every built-in as an owned [`Agent`], with the user's
/// custom agents appended. A custom agent whose id matches a built-in overrides
/// it (the custom entry wins, in built-in position).
pub fn catalog(custom: &[CustomAgent]) -> Vec<Agent> {
    let mut out: Vec<Agent> = BUILTINS.iter().map(AgentDef::to_agent).collect();
    for c in custom {
        let owned = Agent::from_custom(c);
        if let Some(slot) = out.iter_mut().find(|a| a.id == owned.id) {
            *slot = owned;
        } else {
            out.push(owned);
        }
    }
    out
}

/// Resolve one agent id against the catalog (built-ins + custom).
pub fn resolve(id: &str, custom: &[CustomAgent]) -> Option<Agent> {
    catalog(custom).into_iter().find(|a| a.id == id)
}

#[cfg(test)]
#[path = "../tests/registry.rs"]
mod tests;
