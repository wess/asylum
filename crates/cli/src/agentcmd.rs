//! `asylum agent` — the roster: agents that persist between tasks.
//!
//! A run is a thing that happened. A named agent is somebody who keeps
//! happening: a name, a brief, and a growing list of things it learned about
//! this repository. The value is the third one. Every fresh run rediscovers
//! that the integration tests need a database up, that the generated files are
//! not the ones to edit, that the flaky test is flaky — and then the run ends
//! and it is rediscovered again tomorrow.
//!
//! Project-scoped on purpose. A role is about a codebase: "Reviewer" in a Rust
//! workspace and "Reviewer" on a marketing site have nothing useful to say to
//! each other, and merging their memories would make both worse.

use crate::{flag, help, positionals};

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn open() -> Result<store::Db, String> {
    store::Db::open(store::default_path()).map_err(|e| e.to_string())
}

fn project(db: &store::Db, named: Option<&str>) -> Result<store::Project, String> {
    let projects = db.projects().map_err(|e| e.to_string())?;
    match named {
        Some(name) => projects
            .into_iter()
            .find(|p| p.name == name || p.path == name)
            .ok_or_else(|| format!("no project called {name:?}")),
        None => projects
            .into_iter()
            .next()
            .ok_or_else(|| "open a project in Asylum first".to_string()),
    }
}

fn name_arg(args: &[String], path: &[&str]) -> Result<String, String> {
    positionals(args)
        .first()
        .cloned()
        .ok_or_else(|| format!("an agent name is required {}", help::hint(path)))
}

pub fn agent(args: &[String]) -> Result<(), String> {
    let rest = &args[1.min(args.len())..];
    match args.first().map(String::as_str).unwrap_or("list") {
        "list" => list(rest),
        "add" => add(rest),
        "show" => show(rest),
        "remember" => remember(rest),
        "forget" => forget(rest),
        "rm" => rm(rest),
        other => Err(format!(
            "unknown `asylum agent {other}` {}",
            help::hint(&["agent"])
        )),
    }
}

fn list(args: &[String]) -> Result<(), String> {
    let db = open()?;
    let p = project(&db, flag(args, "--project"))?;
    let roster = db.named_agents(p.id).map_err(|e| e.to_string())?;
    if roster.is_empty() {
        println!("no named agents yet (try `asylum agent add Reviewer --role '...'`)");
    }
    for a in roster {
        let learned = a.memory.lines().filter(|l| !l.trim().is_empty()).count();
        println!(
            "{:<16} {:<14} {:>3} learned  {}",
            a.name, a.agent_id, learned, a.role
        );
    }
    Ok(())
}

fn add(args: &[String]) -> Result<(), String> {
    let name = name_arg(args, &["agent", "add"])?;
    // A fan-out entry is matched against the roster first, so a named agent
    // called `codex` would silently shadow codex itself — and then every
    // selection of codex would carry somebody else's memory.
    if agent::registry::resolve(&name, &[]).is_some() {
        return Err(format!(
            "{name:?} is already an agent id — pick a name for the person, not the tool"
        ));
    }
    let db = open()?;
    let p = project(&db, flag(args, "--project"))?;
    let role = flag(args, "--role").unwrap_or("").to_string();
    let agent_id = flag(args, "--agent").unwrap_or("claude-code").to_string();
    let existed = db.named_agent(p.id, &name).is_ok();
    db.save_named_agent(p.id, &name, &role, &agent_id, now())
        .map_err(|e| e.to_string())?;
    if existed {
        // Worth saying explicitly: the obvious fear when re-running this is
        // that you just wiped what it knew.
        println!("updated {name} ({agent_id}) — memory kept");
    } else {
        println!("added {name} ({agent_id})");
    }
    Ok(())
}

fn show(args: &[String]) -> Result<(), String> {
    let name = name_arg(args, &["agent", "show"])?;
    let db = open()?;
    let p = project(&db, flag(args, "--project"))?;
    let a = db
        .named_agent(p.id, &name)
        .map_err(|_| format!("no agent called {name:?}"))?;
    println!("{} — {}", a.name, a.agent_id);
    if !a.role.trim().is_empty() {
        println!("{}", a.role);
    }
    if a.memory.trim().is_empty() {
        println!("\nhasn't learned anything yet");
    } else {
        println!("\nwhat it knows:\n{}", a.memory);
    }
    Ok(())
}

fn remember(args: &[String]) -> Result<(), String> {
    let words = positionals(args);
    let (name, note) = words.split_first().ok_or_else(|| {
        format!(
            "usage: asylum agent remember <name> <fact> {}",
            help::hint(&["agent", "remember"])
        )
    })?;
    let note = note.join(" ");
    if note.trim().is_empty() {
        return Err(format!(
            "a fact is required {}",
            help::hint(&["agent", "remember"])
        ));
    }
    let db = open()?;
    let p = project(&db, flag(args, "--project"))?;
    let a = db
        .named_agent(p.id, name)
        .map_err(|_| format!("no agent called {name:?}"))?;
    if db.remember(a.id, &note).map_err(|e| e.to_string())? {
        println!("{name} will remember: {note}");
    } else {
        println!("{name} already knew that");
    }
    Ok(())
}

fn forget(args: &[String]) -> Result<(), String> {
    let name = name_arg(args, &["agent", "forget"])?;
    let db = open()?;
    let p = project(&db, flag(args, "--project"))?;
    let a = db
        .named_agent(p.id, &name)
        .map_err(|_| format!("no agent called {name:?}"))?;
    db.forget(a.id).map_err(|e| e.to_string())?;
    println!("{name} forgot everything (still on the roster)");
    Ok(())
}

fn rm(args: &[String]) -> Result<(), String> {
    let name = name_arg(args, &["agent", "rm"])?;
    let db = open()?;
    let p = project(&db, flag(args, "--project"))?;
    db.delete_named_agent(p.id, &name)
        .map_err(|e| e.to_string())?;
    println!("removed {name} (its past runs are kept)");
    Ok(())
}
