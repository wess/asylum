//! Per-command help text and the tightened `asylum help` overview.
//!
//! A single table of [`Topic`]s (plain data) backs three entry points in
//! `main.rs`: `asylum <command> --help` / `-h`, `asylum help <command>
//! [<subcommand>]`, and the usage-error hints printed on bad input. There is
//! one topic per top-level dispatcher command (`path.len() == 1`) and one per
//! documented nested subcommand (`path.len() == 2`, e.g. `["control",
//! "status"]`). `completions.rs` reads the same table to generate shell
//! completion scripts, so the command tree only lives in one place.

pub(crate) struct Topic {
    /// `["worktree"]` or `["worktree", "create"]`.
    pub path: &'static [&'static str],
    /// Overview grouping; only meaningful for top-level topics.
    pub group: Option<&'static str>,
    pub summary: &'static str,
    pub usage: &'static [&'static str],
    /// `(token, explanation)` pairs, rendered as an aligned table.
    pub params: &'static [(&'static str, &'static str)],
    /// Free-text callouts for behavior that isn't obvious from usage alone.
    pub notes: &'static [&'static str],
    pub examples: &'static [&'static str],
}

/// Group headings for the overview, in display order.
const GROUPS: &[&str] = &[
    "worktrees & runs",
    "fleet control",
    "secrets",
    "plugins",
    "computer use",
    "other",
];

pub(crate) const TOPICS: &[Topic] = &[
    // ---- worktree ----------------------------------------------------
    Topic {
        path: &["worktree"],
        group: Some("worktrees & runs"),
        summary: "create, list, and remove isolated git worktrees",
        usage: &["asylum worktree <create <path> | list | remove <path>> [--repo <dir>]"],
        params: &[
            ("create", "create a new worktree (and branch)"),
            ("list", "list every worktree of the repository"),
            ("remove", "remove a worktree"),
        ],
        notes: &[],
        examples: &["asylum worktree list"],
    },
    Topic {
        path: &["worktree", "create"],
        group: None,
        summary: "create a new git worktree",
        usage: &["asylum worktree create <path> [--branch <name>] [--start <ref>] [--repo <dir>]"],
        params: &[
            ("<path>", "where to create the worktree (relative paths resolve against --repo)"),
            ("--branch <name>", "branch to create (default: derived from the path's last component)"),
            ("--start <ref>", "commit or ref the new branch starts from (default: HEAD)"),
            ("--repo <dir>", "path to the git repository (default: the current directory)"),
        ],
        notes: &[],
        examples: &[
            "asylum worktree create ../fix-login --branch fix-login",
            "asylum worktree create ../hotfix --start origin/main --repo ~/code/asylum",
        ],
    },
    Topic {
        path: &["worktree", "list"],
        group: None,
        summary: "list every worktree of a repository",
        usage: &["asylum worktree list [--repo <dir>]"],
        params: &[("--repo <dir>", "path to the git repository (default: the current directory)")],
        notes: &[],
        examples: &["asylum worktree list", "asylum worktree list --repo ~/code/asylum"],
    },
    Topic {
        path: &["worktree", "remove"],
        group: None,
        summary: "remove a git worktree",
        usage: &["asylum worktree remove <path> [--force] [--repo <dir>]"],
        params: &[
            ("<path>", "worktree path to remove"),
            ("--force", "remove even if the worktree has uncommitted changes"),
            ("--repo <dir>", "path to the git repository (default: the current directory)"),
        ],
        notes: &[],
        examples: &["asylum worktree remove ../fix-login", "asylum worktree remove ../fix-login --force"],
    },
    // ---- run -----------------------------------------------------------
    Topic {
        path: &["run"],
        group: Some("worktrees & runs"),
        summary: "launch a coding agent against a prompt",
        usage: &["asylum run <agent> <prompt...> [--cwd <dir>]"],
        params: &[
            ("<agent>", "registry id of the agent to launch, e.g. codex"),
            ("<prompt...>", "remaining words are joined with spaces into the prompt"),
            ("--cwd <dir>", "working directory for the agent process (default: the current directory)"),
        ],
        notes: &["Waits up to 10 minutes for the agent process to finish, then prints its terminal screen."],
        examples: &[
            "asylum run codex fix the failing test in src/git/worktree.rs",
            "asylum run codex --cwd ../fix-login summarize the diff",
        ],
    },
    // ---- search ----------------------------------------------------------
    Topic {
        path: &["search"],
        group: Some("worktrees & runs"),
        summary: "search file contents across a worktree",
        usage: &["asylum search <pattern> [--dir <dir>]"],
        params: &[
            ("<pattern>", "text or regex pattern to search for"),
            ("--dir <dir>", "directory to search (default: the current directory)"),
        ],
        notes: &["Uses ripgrep when available, falling back to `git grep`."],
        examples: &["asylum search TODO", "asylum search 'fn create' --dir crates/git"],
    },
    // ---- layout ------------------------------------------------------
    Topic {
        path: &["layout"],
        group: Some("worktrees & runs"),
        summary: "inspect the fan-out presets defined in settings.json",
        usage: &["asylum layout <list | show <name>>"],
        params: &[
            ("list", "list every layout"),
            ("show <name>", "show one layout's detail"),
        ],
        notes: &[],
        examples: &["asylum layout list"],
    },
    Topic {
        path: &["layout", "list"],
        group: None,
        summary: "list the fan-out presets defined in settings.json",
        usage: &["asylum layout list"],
        params: &[],
        notes: &[],
        examples: &["asylum layout list"],
    },
    Topic {
        path: &["layout", "show"],
        group: None,
        summary: "show one fan-out preset's detail",
        usage: &["asylum layout show <name>"],
        params: &[("<name>", "layout name, e.g. duel, triad, swarm")],
        notes: &[],
        examples: &["asylum layout show triad"],
    },
    // ---- control -----------------------------------------------------
    Topic {
        path: &["control"],
        group: Some("fleet control"),
        summary: "orchestrate the fleet from inside a running agent",
        usage: &["asylum control <status|read|spawn|activity|check|skill>"],
        params: &[
            ("status", "list every run in this task"),
            ("read <id>", "print a run's recent output"),
            ("spawn <agent> <prompt>", "queue a helper run on this task"),
            ("activity <state>", "report this run's semantic activity"),
            ("check", "queue a checks pass for this run"),
            ("skill", "print the agent-facing skill doc"),
        ],
        notes: &[
            "Only works inside a running agent's worktree - the app injects \
             ASYLUM_CONTROL_URL, ASYLUM_TASK_ID, and ASYLUM_RUN_ID.",
        ],
        examples: &["asylum control status"],
    },
    Topic {
        path: &["control", "status"],
        group: None,
        summary: "list every run in the current task",
        usage: &["asylum control status"],
        params: &[],
        notes: &[],
        examples: &["asylum control status"],
    },
    Topic {
        path: &["control", "read"],
        group: None,
        summary: "print a run's recent output",
        usage: &["asylum control read <run-id>"],
        params: &[("<run-id>", "id of the run whose output to print")],
        notes: &[],
        examples: &["asylum control read 42"],
    },
    Topic {
        path: &["control", "spawn"],
        group: None,
        summary: "queue a helper run on the current task",
        usage: &["asylum control spawn <agent> <prompt...>"],
        params: &[
            ("<agent>", "registry id of the agent to launch, e.g. codex"),
            ("<prompt...>", "remaining words are joined with spaces into the prompt"),
        ],
        notes: &[],
        examples: &["asylum control spawn codex 'add integration tests for search'"],
    },
    Topic {
        path: &["control", "activity"],
        group: None,
        summary: "report this run's semantic activity",
        usage: &["asylum control activity <working|blocked|done|idle>"],
        params: &[("<state>", "one of working, blocked, done, idle")],
        notes: &[],
        examples: &["asylum control activity blocked"],
    },
    Topic {
        path: &["control", "check"],
        group: None,
        summary: "queue a checks pass for the current run",
        usage: &["asylum control check"],
        params: &[],
        notes: &[],
        examples: &["asylum control check"],
    },
    Topic {
        path: &["control", "skill"],
        group: None,
        summary: "print the agent-facing control skill doc",
        usage: &["asylum control skill"],
        params: &[],
        notes: &[],
        examples: &["asylum control skill"],
    },
    // ---- wait --------------------------------------------------------
    Topic {
        path: &["wait"],
        group: Some("fleet control"),
        summary: "block until a run reaches a status or activity",
        usage: &["asylum wait run <id> [--status <s>] [--activity <a>] [--timeout <secs>]"],
        params: &[("run", "the only supported subcommand")],
        notes: &[
            "Needs the same control-surface environment as `asylum control` \
             (ASYLUM_CONTROL_URL).",
        ],
        examples: &["asylum wait run 42 --status succeeded"],
    },
    Topic {
        path: &["wait", "run"],
        group: None,
        summary: "poll a run until it reaches a status or activity",
        usage: &["asylum wait run <id> [--status <s>] [--activity <a>] [--timeout <secs>]"],
        params: &[
            ("<id>", "run id to poll"),
            ("--status <s>", "wait for this status: queued, running, succeeded, failed, cancelled"),
            ("--activity <a>", "wait for this activity: working, blocked, done, idle"),
            ("--timeout <secs>", "give up after this many seconds (default: 600)"),
        ],
        notes: &["At least one of --status or --activity is required."],
        examples: &[
            "asylum wait run 42 --status succeeded",
            "asylum wait run 42 --activity done --timeout 120",
        ],
    },
    // ---- keep --------------------------------------------------------
    Topic {
        path: &["keep"],
        group: Some("secrets"),
        summary: "manage the encrypted secret store",
        usage: &["asylum keep <set <name> | rm <name> | list> [--project <id>]"],
        params: &[
            ("set", "store a secret (value from --value or stdin)"),
            ("rm", "delete a secret (alias: remove)"),
            ("list", "list secret names in scope (alias: ls)"),
        ],
        notes: &["Requires ASYLUM_KEEP_PASSPHRASE in the environment to unlock the keep."],
        examples: &["asylum keep list"],
    },
    Topic {
        path: &["keep", "set"],
        group: None,
        summary: "store a secret in the keep",
        usage: &["asylum keep set <name> [--project <id>] [--value <v>]"],
        params: &[
            ("<name>", "secret name"),
            ("--project <id>", "scope to a project instead of global"),
            ("--value <v>", "the secret value; if omitted, read from stdin"),
        ],
        notes: &[],
        examples: &[
            "asylum keep set openai_api_key --value sk-...",
            "echo -n sk-... | asylum keep set openai_api_key --project 3",
        ],
    },
    Topic {
        path: &["keep", "rm"],
        group: None,
        summary: "delete a secret from the keep",
        usage: &[
            "asylum keep rm <name> [--project <id>]",
            "asylum keep remove <name> [--project <id>]   (alias)",
        ],
        params: &[
            ("<name>", "secret name to delete"),
            ("--project <id>", "scope to a project instead of global"),
        ],
        notes: &[],
        examples: &["asylum keep rm openai_api_key"],
    },
    Topic {
        path: &["keep", "list"],
        group: None,
        summary: "list secret names in scope",
        usage: &["asylum keep list [--project <id>]", "asylum keep ls [--project <id>]   (alias)"],
        params: &[("--project <id>", "scope to a project instead of global")],
        notes: &["Prints names only - never values."],
        examples: &["asylum keep list", "asylum keep list --project 3"],
    },
    // ---- call --------------------------------------------------------
    Topic {
        path: &["call"],
        group: Some("secrets"),
        summary: "make a masked outbound API call through the secrets proxy",
        usage: &[
            "asylum call",
            "asylum call <upstream> <METHOD> <path> [--data <body>|--data @file]",
            "asylum call --skill",
        ],
        params: &[
            ("<upstream>", "configured upstream name (run with no args to list them)"),
            ("<METHOD>", "HTTP method, e.g. GET, POST (default: GET)"),
            ("<path>", "path on the upstream, e.g. /v1/chat/completions"),
            ("--data <body>", "request body; @file reads from a file (alias: -d)"),
            ("--skill", "print the secrets-proxy skill doc instead of making a call"),
        ],
        notes: &[
            "Requires ASYLUM_PROXY_URL / ASYLUM_PROXY_TOKEN, injected automatically \
             inside a run; the agent never sees the upstream's real credential.",
        ],
        examples: &[
            "asylum call",
            "asylum call openai POST /v1/chat/completions --data @payload.json",
        ],
    },
    // ---- plugin --------------------------------------------------------
    Topic {
        path: &["plugin"],
        group: Some("plugins"),
        summary: "install, search, and list plugins",
        usage: &["asylum plugin <install <owner/repo> | search [--limit n] | list>"],
        params: &[
            ("install", "fetch a plugin from GitHub into the plugin directory"),
            ("search", "discover community plugins tagged asylum-plugin"),
            ("list", "list installed plugins"),
        ],
        notes: &[],
        examples: &["asylum plugin list"],
    },
    Topic {
        path: &["plugin", "install"],
        group: None,
        summary: "install a plugin from GitHub",
        usage: &["asylum plugin install <owner/repo>[@ref]"],
        params: &[(
            "<owner/repo>[@ref]",
            "GitHub repo to fetch; an optional @ref pins a branch, tag, or commit",
        )],
        notes: &[],
        examples: &[
            "asylum plugin install wess/asylum-plugin-linear",
            "asylum plugin install wess/asylum-plugin-linear@v1.2.0",
        ],
    },
    Topic {
        path: &["plugin", "search"],
        group: None,
        summary: "discover community plugins on GitHub",
        usage: &["asylum plugin search [--limit <n>]"],
        params: &[("--limit <n>", "maximum results to show (default: 30)")],
        notes: &["Shells out to the GitHub CLI (`gh`) to search the asylum-plugin topic."],
        examples: &["asylum plugin search", "asylum plugin search --limit 10"],
    },
    Topic {
        path: &["plugin", "list"],
        group: None,
        summary: "list installed plugins",
        usage: &["asylum plugin list"],
        params: &[],
        notes: &[],
        examples: &["asylum plugin list"],
    },
    // ---- mcp -------------------------------------------------------------
    Topic {
        path: &["mcp"],
        group: Some("plugins"),
        summary: "the aggregated MCP gateway",
        usage: &["asylum mcp <list | serve [--bind addr] | stdio | skill>"],
        params: &[
            ("list", "tools the running gateway currently exposes"),
            ("serve", "run a standalone gateway from settings.json"),
            ("stdio", "bridge a stdio-only MCP client to the gateway"),
            ("skill", "print the agent-facing skill doc"),
        ],
        notes: &[
            "`list` and `stdio` talk to the gateway via ASYLUM_MCP_URL / ASYLUM_MCP_TOKEN, \
             injected inside a run.",
        ],
        examples: &["asylum mcp list"],
    },
    Topic {
        path: &["mcp", "list"],
        group: None,
        summary: "list tools the running gateway exposes",
        usage: &["asylum mcp list"],
        params: &[],
        notes: &[],
        examples: &["asylum mcp list"],
    },
    Topic {
        path: &["mcp", "serve"],
        group: None,
        summary: "run a standalone MCP gateway from settings.json",
        usage: &["asylum mcp serve [--bind <addr>]"],
        params: &[("--bind <addr>", "address to listen on (default: settings.json's mcp.bind)")],
        notes: &[
            "For an agent launched outside the app. Resolves upstream secrets from \
             the keep when ASYLUM_KEEP_PASSPHRASE is set.",
        ],
        examples: &["asylum mcp serve", "asylum mcp serve --bind 127.0.0.1:4180"],
    },
    Topic {
        path: &["mcp", "stdio"],
        group: None,
        summary: "bridge a stdio-only MCP client to the gateway",
        usage: &["asylum mcp stdio"],
        params: &[],
        notes: &["Reads newline-delimited JSON-RPC from stdin and forwards it to the gateway over HTTP."],
        examples: &["asylum mcp stdio"],
    },
    Topic {
        path: &["mcp", "skill"],
        group: None,
        summary: "print the agent-facing MCP gateway skill doc",
        usage: &["asylum mcp skill"],
        params: &[],
        notes: &[],
        examples: &["asylum mcp skill"],
    },
    // ---- computer use --------------------------------------------------
    Topic {
        path: &["snapshot"],
        group: Some("computer use"),
        summary: "screenshot the desktop",
        usage: &["asylum snapshot [<out.png>]"],
        params: &[("<out.png>", "output file path (default: asylum-snapshot.png)")],
        notes: &["macOS uses screencapture; Linux uses scrot."],
        examples: &["asylum snapshot", "asylum snapshot ~/Desktop/before.png"],
    },
    Topic {
        path: &["click"],
        group: Some("computer use"),
        summary: "click the desktop at (x, y)",
        usage: &["asylum click <x> <y>"],
        params: &[
            ("<x>", "horizontal pixel coordinate"),
            ("<y>", "vertical pixel coordinate"),
        ],
        notes: &["macOS uses cliclick; Linux uses xdotool."],
        examples: &["asylum click 640 480"],
    },
    Topic {
        path: &["fill"],
        group: Some("computer use"),
        summary: "type text into the focused window",
        usage: &["asylum fill <text...>"],
        params: &[("<text...>", "words are joined with spaces and typed as keystrokes")],
        notes: &["macOS uses osascript keystroke; Linux uses xdotool type."],
        examples: &["asylum fill hello world"],
    },
    // ---- other -------------------------------------------------------
    Topic {
        path: &["completions"],
        group: Some("other"),
        summary: "print a shell completion script",
        usage: &["asylum completions <bash|zsh|fish>"],
        params: &[
            ("bash", "source it from ~/.bashrc, or place it in your bash-completion.d directory"),
            ("zsh", "save it as `_asylum` on your $fpath, then run `compinit`"),
            ("fish", "save it as ~/.config/fish/completions/asylum.fish"),
        ],
        notes: &[],
        examples: &[
            "asylum completions zsh > ~/.zsh/completions/_asylum",
            "asylum completions bash | sudo tee /etc/bash_completion.d/asylum",
        ],
    },
    Topic {
        path: &["version"],
        group: Some("other"),
        summary: "print the asylum version",
        usage: &["asylum version", "asylum --version", "asylum -V"],
        params: &[],
        notes: &[],
        examples: &["asylum version"],
    },
    Topic {
        path: &["help"],
        group: Some("other"),
        summary: "show this message, or help for one command",
        usage: &[
            "asylum help",
            "asylum help <command> [<subcommand>]",
            "asylum <command> --help",
        ],
        params: &[
            ("<command>", "print the focused help block for this command"),
            ("<subcommand>", "print help for a nested subcommand, e.g. control status"),
        ],
        notes: &[],
        examples: &["asylum help", "asylum help worktree create"],
    },
];

/// Exact-match lookup, truncating `path` to the 1- or 2-element shape every
/// topic is keyed by (so a trailing positional like a run id doesn't break
/// the lookup).
pub(crate) fn lookup(path: &[&str]) -> Option<&'static Topic> {
    let want = &path[..path.len().min(2)];
    TOPICS.iter().find(|t| t.path == want)
}

/// Resolve the topic for `asylum <cmd> [<sub>] ... --help`: prefer a nested
/// topic when the first token after `cmd` names one, else the command's own
/// overview. Flag-like tokens (`--help`, `-h`, ...) are never mistaken for a
/// nested subcommand name.
pub(crate) fn for_invocation(cmd: &str, rest: &[String]) -> Option<&'static Topic> {
    if let Some(sub) = rest.first().filter(|s| !s.starts_with('-')) {
        if let Some(topic) = lookup(&[cmd, sub.as_str()]) {
            return Some(topic);
        }
    }
    lookup(&[cmd])
}

/// A short pointer at the end of a usage error, e.g.
/// `(see \`asylum worktree create --help\`)`.
pub(crate) fn hint(path: &[&str]) -> String {
    format!("(see `asylum {} --help`)", path.join(" "))
}

/// Render one topic as a focused help block.
pub(crate) fn render(t: &Topic) -> String {
    let mut out = format!("asylum {} - {}\n\nUSAGE:\n", t.path.join(" "), t.summary);
    for line in t.usage {
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }
    if !t.params.is_empty() {
        out.push_str("\nARGS:\n");
        let width = t
            .params
            .iter()
            .map(|(k, _)| k.chars().count())
            .max()
            .unwrap_or(0);
        for (k, v) in t.params {
            out.push_str(&format!("  {k:<width$}  {v}\n"));
        }
    }
    if !t.notes.is_empty() {
        out.push_str("\nNOTES:\n");
        for n in t.notes {
            out.push_str("  ");
            out.push_str(n);
            out.push('\n');
        }
    }
    if !t.examples.is_empty() {
        out.push_str("\nEXAMPLES:\n");
        for e in t.examples {
            out.push_str("  ");
            out.push_str(e);
            out.push('\n');
        }
    }
    out
}

/// The tightened `asylum help` overview: top-level commands grouped by theme.
pub(crate) fn overview() -> String {
    let mut out = String::from(
        "asylum — Agent Development Environment CLI\n\n\
         USAGE:\n\
         \x20 asylum <command> [args] [flags]\n\
         \x20 asylum <command> --help          detailed help for one command\n\
         \x20 asylum help <command> [<sub>]    same, via the help command\n\n",
    );
    for group in GROUPS {
        let tops: Vec<&Topic> = TOPICS
            .iter()
            .filter(|t| t.path.len() == 1 && t.group == Some(*group))
            .collect();
        if tops.is_empty() {
            continue;
        }
        out.push_str(&group.to_uppercase());
        out.push_str(":\n");
        let width = tops.iter().map(|t| t.path[0].len()).max().unwrap_or(0);
        for t in tops {
            out.push_str(&format!("  {:<width$}  {}\n", t.path[0], t.summary));
        }
        out.push('\n');
    }
    out.push_str(
        "Run `asylum <command> --help` for the full picture, or `asylum completions \
         <shell>` to set up tab completion.\n",
    );
    out
}

#[cfg(test)]
#[path = "../tests/help.rs"]
mod tests;
