//! The `asylum` CLI - script the ADE from the shell or an agent.
//!
//! ```text
//! asylum worktree create <path> [--branch <b>] [--start <ref>] [--repo <dir>]
//! asylum worktree list [--repo <dir>]
//! asylum worktree remove <path> [--repo <dir>] [--force]
//! asylum run <agent> <prompt...> [--cwd <dir>]
//! asylum search <pattern> [--dir <dir>]
//! asylum snapshot [<out.png>]          # computer use: screenshot
//! asylum click <x> <y>                 # computer use: mouse click
//! asylum fill <text...>                # computer use: type text
//! ```

mod computer;

use std::path::{Path, PathBuf};
use std::process::exit;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    let rest = &args[1..];

    let result = match cmd {
        "worktree" => worktree(rest),
        "run" => run_agent(rest),
        "search" => do_search(rest),
        "snapshot" => snapshot(rest),
        "click" => click(rest),
        "fill" => fill(rest),
        "--version" | "-V" | "version" => {
            println!("asylum {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command `{other}` (try `asylum help`)")),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        exit(1);
    }
}

/// Value of `--flag <value>`; returns None if absent.
fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// Positional args (those not consumed by `--flag value` pairs and not flags).
fn positionals(args: &[String]) -> Vec<String> {
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
    let rest = &args[1..];
    let repo = repo_dir(rest);
    match sub {
        "create" => {
            let pos = positionals(rest);
            let path = pos.first().ok_or("usage: asylum worktree create <path>")?;
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
            let path = pos.first().ok_or("usage: asylum worktree remove <path>")?;
            git::worktree::remove(&repo, Path::new(path), has_flag(rest, "--force"))
                .map_err(|e| e.to_string())?;
            println!("removed {path}");
            Ok(())
        }
        _ => Err("usage: asylum worktree <create|list|remove>".into()),
    }
}

fn run_agent(args: &[String]) -> Result<(), String> {
    let pos = positionals(args);
    let agent_id = pos.first().ok_or("usage: asylum run <agent> <prompt...>")?;
    let prompt = pos[1..].join(" ");
    if prompt.is_empty() {
        return Err("a prompt is required".into());
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
    let pattern = pos.first().ok_or("usage: asylum search <pattern>")?;
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
    let x: i32 = pos
        .first()
        .and_then(|s| s.parse().ok())
        .ok_or("usage: asylum click <x> <y>")?;
    let y: i32 = pos
        .get(1)
        .and_then(|s| s.parse().ok())
        .ok_or("usage: asylum click <x> <y>")?;
    let (program, argv) = computer::click_command(std::env::consts::OS, x, y);
    computer::run(&program, &argv)
}

fn fill(args: &[String]) -> Result<(), String> {
    let text = positionals(args).join(" ");
    if text.is_empty() {
        return Err("usage: asylum fill <text...>".into());
    }
    let (program, argv) = computer::fill_command(std::env::consts::OS, &text);
    computer::run(&program, &argv)
}

fn print_help() {
    println!(
        "asylum — Agent Development Environment CLI\n\n\
         USAGE:\n\
         \x20 asylum worktree create <path> [--branch <b>] [--start <ref>] [--repo <dir>]\n\
         \x20 asylum worktree list [--repo <dir>]\n\
         \x20 asylum worktree remove <path> [--force] [--repo <dir>]\n\
         \x20 asylum run <agent> <prompt...> [--cwd <dir>]\n\
         \x20 asylum search <pattern> [--dir <dir>]\n\
         \x20 asylum snapshot [<out.png>]\n\
         \x20 asylum click <x> <y>\n\
         \x20 asylum fill <text...>\n"
    );
}
