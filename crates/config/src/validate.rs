//! Semantic validation.
//!
//! `salvage` (see `load.rs`) only catches values whose *type* is wrong. A
//! value can deserialize cleanly and still be nonsense: a bind string with no
//! port, two servers sharing one port, a worktree directory that is empty, a
//! layout naming an agent id nobody has heard of. This module is the second
//! pass: it runs after a load has already produced a well-typed `Settings`
//! (or `ProjectConfig`), looks for exactly that shape of problem, and turns
//! each one into a `Diagnostic` alongside the value it came from.
//!
//! None of these problems abort the load, matching `salvage`'s rule: the app
//! always gets a usable value. But unlike a bad *type*, a bad-but-typed value
//! cannot always be dropped in favor of the compiled default without a
//! judgment call - an unparseable bind fails loudly and safely the moment
//! something tries to use it, so it is left alone (warned, then used as-is);
//! an empty `worktree_dir` fails silently and dangerously (every worktree
//! path becomes root-relative), so it is replaced (warned, then defaulted).
//! Each rule below states which it picked and why.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use crate::model::Settings;
use crate::project::ProjectConfig;
use crate::Diagnostic;

/// Run every semantic rule against an already-typed `Settings`, returning one
/// `Diagnostic` per problem found. Wired into the tail of `load::load_str`,
/// after `salvage` has already turned type errors into diagnostics, so this
/// pass only ever sees values that deserialized cleanly.
pub(crate) fn validate(settings: &mut Settings) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    validate_binds(settings, &mut out);
    validate_worktree_dir(settings, &mut out);
    validate_numerics(settings, &mut out);
    validate_layouts(settings, &mut out);
    validate_custom_agents(settings, &mut out);
    out
}

/// The same pass for the per-project `asylum.toml`, run from
/// `project::parse_project` right after a clean TOML parse.
pub(crate) fn validate_project(cfg: &mut ProjectConfig) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if let Some(branch) = cfg.base_branch.as_deref() {
        if let Some(problem) = branch_name_problem(branch) {
            out.push(Diagnostic::new(
                "base_branch",
                format!(
                    "base_branch '{branch}' {problem}; cleared so the \
                     project's actual base branch is used instead"
                ),
            ));
            cfg.base_branch = None;
        }
    }
    out
}

// ── Server binds ────────────────────────────────────────────────────────────

/// The four loopback servers whose bind settings are checked, paired with the
/// dotted key path used in diagnostics.
fn server_binds(settings: &Settings) -> [(&'static str, &str); 4] {
    [
        ("companion.bind", settings.companion.bind.as_str()),
        ("control.bind", settings.control.bind.as_str()),
        ("proxy.bind", settings.proxy.bind.as_str()),
        ("mcp.bind", settings.mcp.bind.as_str()),
    ]
}

/// Parse `bind`'s port if it has the syntactic shape of a socket address:
/// some non-empty host followed by `:` and a `u16` port. Looser than
/// `crate::bind::guard`, which resolves the host - including DNS for a
/// hostname - to decide loopback-ness at actual startup; this runs on every
/// settings load, so it stays offline and checks shape only. A literal IP is
/// confirmed with `SocketAddr`; anything else falls back to a plain
/// `host:port` split so a legitimate hostname bind (`localhost:8787`) is not
/// flagged.
fn bind_port(bind: &str) -> Option<u16> {
    if let Ok(addr) = bind.parse::<std::net::SocketAddr>() {
        return Some(addr.port());
    }
    let (host, port) = bind.rsplit_once(':')?;
    if host.trim().is_empty() {
        return None;
    }
    port.parse::<u16>().ok()
}

/// Bind shape and cross-server port collisions, checked regardless of each
/// server's `enabled` flag: the point is to catch the mistake before the user
/// flips a server on, not after. Both problems are warned and the value is
/// kept, because an unparseable or colliding bind fails loudly the moment the
/// server tries to start (see `bind::guard`); overwriting the user's choice
/// here would only hide what they typed without fixing anything.
fn validate_binds(settings: &Settings, out: &mut Vec<Diagnostic>) {
    let mut ports: BTreeMap<u16, Vec<&'static str>> = BTreeMap::new();
    for (key, bind) in server_binds(settings) {
        match bind_port(bind) {
            None => out.push(Diagnostic::new(
                key,
                format!(
                    "{key} '{bind}' does not look like a host:port address; \
                     the server will fail to start"
                ),
            )),
            // Port 0 asks the OS for a free ephemeral port; two servers both
            // set to 0 still land on different real ports, so it can never
            // actually collide.
            Some(0) => {}
            Some(port) => ports.entry(port).or_default().push(key),
        }
    }
    for (port, keys) in ports.into_iter().filter(|(_, keys)| keys.len() > 1) {
        let names = keys.join(", ");
        for key in keys {
            out.push(Diagnostic::new(
                key,
                format!("{names} all bind port {port}; only one of them can start"),
            ));
        }
    }
}

// ── worktree_dir ─────────────────────────────────────────────────────────────

const WORKTREE_DIR_KEY: &str = "worktree_dir";

/// A path-shape predicate paired with the message to report when it matches.
type PathShapeRule = (fn(&str) -> bool, &'static str);

/// Shape checks for `worktree_dir` that catch a value which is not sensibly a
/// path on this platform. Not exhaustive - `Path` accepts nearly any byte
/// sequence as a relative path component - just the mistakes a hand-typed
/// value is likely to make.
const PATH_SHAPE_RULES: &[PathShapeRule] = &[
    (
        |p| p.contains('\0'),
        "contains a NUL byte, which no filesystem accepts",
    ),
    (
        |p| p.starts_with('~'),
        "starts with '~', which is not expanded to a home directory here; \
         use an absolute path or one relative to the project root",
    ),
    (
        |p| p.contains('\\'),
        "contains a backslash; this platform's path separator is '/', so a \
         backslash becomes part of a literal directory name instead of \
         separating one",
    ),
    (
        |p| p.contains("://"),
        "looks like a URL, not a filesystem path",
    ),
];

/// `worktree_dir` is empty, an unlikely path shape, or an existing file.
/// Empty is defaulted: `agent::plan::fanout` builds every worktree path as
/// `{worktree_dir}/{slug}`, so an empty value silently turns every worktree
/// into a root-relative path (`/task-1-claude-code`) outside the project -
/// wrong and dangerous without any loud failure to reveal it. The other two
/// are warned and left as-is: a bad shape or an occupied path fails loudly
/// the moment `git worktree add` runs against it.
fn validate_worktree_dir(settings: &mut Settings, out: &mut Vec<Diagnostic>) {
    let dir = settings.worktree_dir.trim();
    if dir.is_empty() {
        out.push(Diagnostic::new(
            WORKTREE_DIR_KEY,
            "worktree_dir is empty; every worktree path would become \
             root-relative, so the default is used instead",
        ));
        settings.worktree_dir = Settings::default().worktree_dir;
        return;
    }
    if let Some((_, msg)) = PATH_SHAPE_RULES.iter().find(|(check, _)| check(dir)) {
        out.push(Diagnostic::new(
            WORKTREE_DIR_KEY,
            format!("worktree_dir '{dir}' {msg}"),
        ));
        return;
    }
    // A relative worktree_dir is resolved per-project against that project's
    // repo root (`git::worktree::resolve`), which this crate has no notion
    // of, so the existing-file probe would be checked against the wrong base
    // (the process's current directory) and only runs for an absolute path.
    let path = Path::new(dir);
    if path.is_absolute() && path.is_file() {
        out.push(Diagnostic::new(
            WORKTREE_DIR_KEY,
            format!(
                "worktree_dir '{dir}' is an existing file; git cannot create worktrees under it"
            ),
        ));
    }
}

// ── Concurrency / timeout numerics ──────────────────────────────────────────

/// Running more than this many agent processes at once is never intentional.
const MAX_PARALLEL_RUNS_CEILING: u32 = 256;
/// Seven days; a run left going that long is not "timing out gracefully."
const RUN_TIMEOUT_CEILING_MINUTES: u32 = 10_080;

/// `max_parallel_runs` and `run_timeout_minutes` both document `0` as a
/// deliberate sentinel ("unlimited" / "no timeout"), so it is never flagged -
/// only an implausibly large value is a problem, and it is warned and bounded
/// back to the compiled default: nothing downstream treats an absurd
/// concurrency or timeout number as a loud, obvious failure, so left alone it
/// would just make the app behave strangely (or not throttle at all) with no
/// clear signal why.
fn validate_numerics(settings: &mut Settings, out: &mut Vec<Diagnostic>) {
    if settings.max_parallel_runs > MAX_PARALLEL_RUNS_CEILING {
        out.push(Diagnostic::new(
            "max_parallel_runs",
            format!(
                "max_parallel_runs {} exceeds the sane ceiling of {MAX_PARALLEL_RUNS_CEILING}; reset to the default",
                settings.max_parallel_runs
            ),
        ));
        settings.max_parallel_runs = Settings::default().max_parallel_runs;
    }
    if settings.run_timeout_minutes > RUN_TIMEOUT_CEILING_MINUTES {
        out.push(Diagnostic::new(
            "run_timeout_minutes",
            format!(
                "run_timeout_minutes {} exceeds the sane ceiling of {RUN_TIMEOUT_CEILING_MINUTES} ({} days); reset to the default",
                settings.run_timeout_minutes,
                RUN_TIMEOUT_CEILING_MINUTES / 1_440
            ),
        ));
        settings.run_timeout_minutes = Settings::default().run_timeout_minutes;
    }
}

// ── Layouts ──────────────────────────────────────────────────────────────────

/// Ceiling for a single layout's `concurrency` override - see
/// `MAX_PARALLEL_RUNS_CEILING`.
const LAYOUT_CONCURRENCY_CEILING: u32 = 256;

/// Mirrors the ids in `agent::registry::BUILTINS`. `config` sits below
/// `agent` in the workspace's dependency graph (`agent` depends on `config`,
/// not the reverse), so it cannot import the live registry to check against
/// it - this is a frozen copy of just the ids, current as of this writing.
/// Keep it in sync by hand if the registry's id set changes. Staleness here
/// is safe by construction: an id missing from this list is reported but the
/// layout keeps it (see `validate_layouts`), so a false positive costs a
/// stale warning, never a broken layout.
const KNOWN_BUILTIN_AGENT_IDS: &[&str] = &[
    "claude-code",
    "codex",
    "opencode",
    "gemini",
    "grok",
    "cursor-agent",
    "copilot",
    "aider",
    "continue",
    "cline",
    "goose",
    "amp",
    "droid",
    "qwen-code",
    "kimi",
    "kilocode",
    "kiro",
    "codebuff",
    "mistral-vibe",
    "pi",
    "oh-my-pi",
    "hermes",
    "devin",
    "auggie",
    "autohand",
    "charm",
    "command-code",
    "rovo-dev",
    "mimo-code",
    "openclaude",
    "antigravity",
];

fn layout_label(name: &str, index: usize) -> String {
    if name.trim().is_empty() {
        format!("#{index}")
    } else {
        name.to_string()
    }
}

/// Each layout's agent ids and its `concurrency` override.
///
/// An unknown agent id is warned and kept: this crate's view of "known" ids
/// is the frozen list above plus `custom_agents`, which can lag the real
/// registry (a newer built-in, or a custom agent added through a path other
/// than settings.json), so dropping the id could silently shrink a layout
/// that is actually fine. An absurd `concurrency` is warned and reset to `0`
/// ("defer to the global limit") rather than to some arbitrary number - `0`
/// is already the documented sentinel for "no per-layout override."
fn validate_layouts(settings: &mut Settings, out: &mut Vec<Diagnostic>) {
    let custom_ids: BTreeSet<String> = settings
        .custom_agents
        .iter()
        .map(|a| a.id.clone())
        .collect();

    for (i, layout) in settings.layouts.iter_mut().enumerate() {
        let label = layout_label(&layout.name, i);

        let unknown: Vec<String> = layout
            .agents
            .iter()
            .filter(|id| {
                !KNOWN_BUILTIN_AGENT_IDS.contains(&id.as_str()) && !custom_ids.contains(id.as_str())
            })
            .cloned()
            .collect();
        if !unknown.is_empty() {
            out.push(Diagnostic::new(
                format!("layouts[{i}].agents"),
                format!(
                    "layout '{label}' references unknown agent id(s) {}; kept in \
                     case the id is newer than this check, but it will not run \
                     unless one is added",
                    unknown.join(", ")
                ),
            ));
        }

        if layout.concurrency > LAYOUT_CONCURRENCY_CEILING {
            out.push(Diagnostic::new(
                format!("layouts[{i}].concurrency"),
                format!(
                    "layout '{label}' concurrency {} exceeds the sane ceiling of \
                     {LAYOUT_CONCURRENCY_CEILING}; reset to defer to the global limit",
                    layout.concurrency
                ),
            ));
            layout.concurrency = 0;
        }
    }
}

// ── Custom agents ────────────────────────────────────────────────────────────

/// An empty `id` or `program` on a custom agent. Both are warned and kept:
/// `CustomAgent::default()` is itself all-empty, so there is no other default
/// to fall back to - resetting a field to "default" would be a no-op. Left
/// as-is, the entry either cannot be referenced (empty `id`) or fails to
/// launch with a clear process-spawn error the moment it is selected (empty
/// `program`); either way the failure is loud, not silent.
fn validate_custom_agents(settings: &Settings, out: &mut Vec<Diagnostic>) {
    for (i, agent) in settings.custom_agents.iter().enumerate() {
        if agent.id.trim().is_empty() {
            out.push(Diagnostic::new(
                format!("custom_agents[{i}].id"),
                format!(
                    "custom_agents[{i}] has an empty id; it cannot be referenced \
                     by a layout or default_agents"
                ),
            ));
        }
        if agent.program.trim().is_empty() {
            let label = layout_label(&agent.id, i);
            out.push(Diagnostic::new(
                format!("custom_agents[{i}].program"),
                format!("custom agent '{label}' has an empty program and will fail to launch"),
            ));
        }
    }
}

// ── Branch names ─────────────────────────────────────────────────────────────

/// A subset of what `git check-ref-format` forbids in a ref name - enough to
/// catch a hand-typed mistake, not a full reimplementation.
fn branch_name_problem(name: &str) -> Option<&'static str> {
    if name.trim().is_empty() {
        return Some("is empty");
    }
    if name.starts_with('/') || name.ends_with('/') {
        return Some("cannot start or end with '/'");
    }
    if name.starts_with('.') || name.ends_with('.') {
        return Some("cannot start or end with '.'");
    }
    if name.ends_with(".lock") {
        return Some("cannot end with '.lock'");
    }
    if name.contains("..") {
        return Some("cannot contain '..'");
    }
    if name.contains("//") {
        return Some("cannot contain '//'");
    }
    if name.contains("@{") {
        return Some("cannot contain '@{'");
    }
    if name == "@" {
        return Some("cannot be '@'");
    }
    if name
        .chars()
        .any(|c| c.is_control() || " ~^:?*[\\".contains(c))
    {
        return Some(
            "contains a character git forbids in ref names (space, ~, ^, :, ?, *, [, \\, or a control character)",
        );
    }
    None
}

#[cfg(test)]
#[path = "../tests/validate.rs"]
mod tests;
