//! The `asylum` CLI - script the ADE from the shell or an agent.
//!
//! ```text
//! asylum worktree create <path> [--branch <b>] [--start <ref>] [--repo <dir>]
//! asylum worktree list [--repo <dir>]
//! asylum worktree remove <path> [--force] [--repo <dir>]
//! asylum run <agent> <prompt...> [--cwd <dir>]
//! asylum search <pattern> [--dir <dir>]
//! asylum control <status|read|spawn|activity|check|skill>   # orchestrate the fleet
//! asylum call [<upstream> <METHOD> <path> [--data <body>]] [--skill]  # masked API calls
//! asylum mcp <list|serve|stdio|skill>          # the aggregated MCP gateway
//! asylum keep <set <name> [--project <id>] [--value <v>] | rm <name> | list>
//! asylum wait run <id> [--status <s>] [--activity <a>] [--timeout <secs>]
//! asylum plugin <install <owner/repo> | search | list>
//! asylum layout <list | show <name>>
//! asylum snapshot [<out.png>]          # computer use: screenshot
//! asylum click <x> <y>                 # computer use: mouse click
//! asylum fill <text...>                # computer use: type text
//! asylum completions <bash|zsh|fish>   # shell completion scripts
//! ```
//!
//! Every command takes `--help` / `-h` for a focused help block (equivalently
//! `asylum help <command> [<subcommand>]`); `asylum help` alone prints the
//! grouped overview. See `help.rs` for the help table (also the source of
//! truth `completions.rs` draws the completion scripts from).

mod call;
mod completions;
mod computer;
mod ctl;
mod help;
mod keepcmd;
mod layouts;
mod mcpcmd;
mod plugins;

use std::path::{Path, PathBuf};
use std::process::exit;
use std::time::Duration;

/// A top-level command handler: the args after the command name in, a
/// user-facing error out.
type Handler = fn(&[String]) -> Result<(), String>;

/// The dispatch table - every subcommand `asylum` understands. This is the
/// single source of truth for "what commands exist": `completions.rs` reads
/// the (equivalent) top-level list out of `help::TOPICS`, and
/// `tests/main.rs` asserts every entry here has a matching help topic, so a
/// new command added here without help fails `cargo test`.
const COMMANDS: &[(&str, Handler)] = &[
    ("worktree", worktree),
    ("run", run_agent),
    ("search", do_search),
    ("control", ctl::control),
    ("call", call::call),
    ("mcp", mcpcmd::mcp),
    ("keep", keepcmd::keep),
    ("wait", ctl::wait),
    ("plugin", plugins::plugin),
    ("layout", layouts::layout),
    ("snapshot", snapshot),
    ("click", click),
    ("fill", fill),
    ("completions", completions::run),
    ("version", version_cmd),
    ("help", help_cmd),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let raw = args.first().map(String::as_str).unwrap_or("help");
    let cmd = resolve_alias(raw);
    let rest = &args[1.min(args.len())..];

    // `asylum <cmd> [<sub>] --help` (or `-h`) short-circuits to the help
    // block instead of running the command. `asylum help ...` is handled by
    // dispatching straight to `help_cmd` below, since it is itself a command.
    let result = if cmd != "help" && (has_flag(rest, "--help") || has_flag(rest, "-h")) {
        match help::for_invocation(cmd, rest) {
            Some(topic) => {
                print!("{}", help::render(topic));
                Ok(())
            }
            // `cmd` isn't a real command either - fall through to the normal
            // "unknown command" error rather than inventing a new message.
            None => dispatch(cmd, rest),
        }
    } else {
        dispatch(cmd, rest)
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        exit(1);
    }
}

/// `-V`/`--version` and `-h`/`--help` are aliases for the `version`/`help`
/// commands; everything else passes through unchanged.
fn resolve_alias(cmd: &str) -> &str {
    match cmd {
        "-V" | "--version" => "version",
        "-h" | "--help" => "help",
        other => other,
    }
}

fn dispatch(cmd: &str, rest: &[String]) -> Result<(), String> {
    match COMMANDS.iter().find(|(name, _)| *name == cmd) {
        Some((_, handler)) => handler(rest),
        None => Err(format!("unknown command `{cmd}` (try `asylum help`)")),
    }
}

/// Value of `--flag <value>`; returns None if absent.
pub(crate) fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

pub(crate) fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// Positional args (those not consumed by `--flag value` pairs and not flags).
pub(crate) fn positionals(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
            continue;
        }
        if a.starts_with("--") {
            // --force is boolean (takes no value); every other flag takes one.
            if a != "--force" {
                skip = true;
            }
            continue;
        }
        out.push(a.clone());
    }
    out
}

fn repo_dir(args: &[String]) -> PathBuf {
    flag(args, "--repo")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn worktree(args: &[String]) -> Result<(), String> {
    let sub = args.first().map(String::as_str).unwrap_or("");
    let rest = &args[1.min(args.len())..];
    let repo = repo_dir(rest);
    match sub {
        "create" => {
            let pos = positionals(rest);
            let path = pos.first().ok_or_else(|| {
                format!(
                    "usage: asylum worktree create <path> {}",
                    help::hint(&["worktree", "create"])
                )
            })?;
            let branch = flag(rest, "--branch");
            let start = flag(rest, "--start");
            let created =
                git::worktree::create(&repo, path, branch, start).map_err(|e| e.to_string())?;
            println!("{}", created.display());
            Ok(())
        }
        "list" => {
            for w in git::worktree::list(&repo).map_err(|e| e.to_string())? {
                let branch = w.branch.as_deref().unwrap_or("(detached)");
                let mark = if w.primary { "*" } else { " " };
                println!("{mark} {}\t{branch}", w.path.display());
            }
            Ok(())
        }
        "remove" => {
            let pos = positionals(rest);
            let path = pos.first().ok_or_else(|| {
                format!(
                    "usage: asylum worktree remove <path> {}",
                    help::hint(&["worktree", "remove"])
                )
            })?;
            git::worktree::remove(&repo, Path::new(path), has_flag(rest, "--force"))
                .map_err(|e| e.to_string())?;
            println!("removed {path}");
            Ok(())
        }
        _ => Err(format!(
            "usage: asylum worktree <create|list|remove> {}",
            help::hint(&["worktree"])
        )),
    }
}

fn run_agent(args: &[String]) -> Result<(), String> {
    let pos = positionals(args);
    let agent_id = pos.first().ok_or_else(|| {
        format!(
            "usage: asylum run <agent> <prompt...> {}",
            help::hint(&["run"])
        )
    })?;
    let prompt = pos[1..].join(" ");
    if prompt.is_empty() {
        return Err(format!("a prompt is required {}", help::hint(&["run"])));
    }
    let def = agent::find(agent_id).ok_or_else(|| format!("unknown agent `{agent_id}`"))?;
    let cwd = flag(args, "--cwd").unwrap_or(".");
    let spec = agent::command::build(&def.to_agent(), None, &prompt, cwd);
    eprintln!("$ {}", spec.preview());

    let run = runner::Runner::start(&spec).map_err(|e| format!("could not start agent: {e}"))?;
    let state = run.wait(Duration::from_secs(600));
    print!("{}", run.screen_text());
    println!();
    let code = state.exit_code().unwrap_or(0);
    run.shutdown();
    if code != 0 {
        exit(code);
    }
    Ok(())
}

fn do_search(args: &[String]) -> Result<(), String> {
    let pos = positionals(args);
    let pattern = pos
        .first()
        .ok_or_else(|| format!("usage: asylum search <pattern> {}", help::hint(&["search"])))?;
    let dir = flag(args, "--dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let matches =
        search::search(&dir, pattern, &search::Options::default()).map_err(|e| e.to_string())?;
    for m in &matches {
        println!("{}:{}:{}: {}", m.file, m.line, m.column, m.text);
    }
    eprintln!("{} matches", matches.len());
    Ok(())
}

fn snapshot(args: &[String]) -> Result<(), String> {
    let out = positionals(args)
        .first()
        .cloned()
        .unwrap_or_else(|| "asylum-snapshot.png".into());
    let (program, argv) = computer::snapshot_command(std::env::consts::OS, &out);
    computer::run(&program, &argv)?;
    println!("{out}");
    Ok(())
}

fn click(args: &[String]) -> Result<(), String> {
    let pos = positionals(args);
    let usage = || format!("usage: asylum click <x> <y> {}", help::hint(&["click"]));
    let x: i32 = pos.first().and_then(|s| s.parse().ok()).ok_or_else(usage)?;
    let y: i32 = pos.get(1).and_then(|s| s.parse().ok()).ok_or_else(usage)?;
    let (program, argv) = computer::click_command(std::env::consts::OS, x, y);
    computer::run(&program, &argv)
}

fn fill(args: &[String]) -> Result<(), String> {
    let text = positionals(args).join(" ");
    if text.is_empty() {
        return Err(format!(
            "usage: asylum fill <text...> {}",
            help::hint(&["fill"])
        ));
    }
    let (program, argv) = computer::fill_command(std::env::consts::OS, &text);
    computer::run(&program, &argv)
}

fn version_cmd(_args: &[String]) -> Result<(), String> {
    println!("asylum {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}

/// `asylum help [<command> [<subcommand>]]` - the overview when bare, else
/// the same block `asylum <command> --help` would print.
fn help_cmd(args: &[String]) -> Result<(), String> {
    let path = positionals(args);
    if path.is_empty() {
        print!("{}", help::overview());
        return Ok(());
    }
    let path: Vec<&str> = path.iter().map(String::as_str).collect();
    match help::lookup(&path) {
        Some(topic) => {
            print!("{}", help::render(topic));
            Ok(())
        }
        None => Err(format!(
            "no help for `{}` (try `asylum help`)",
            path.join(" ")
        )),
    }
}

#[cfg(test)]
#[path = "../tests/main.rs"]
mod tests;
