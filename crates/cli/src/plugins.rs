//! `asylum plugin` - install plugins from GitHub and discover community ones.

use std::process::Command;

use crate::{flag, positionals};

pub fn plugin(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str).unwrap_or("") {
        "install" => install(&args[1..]),
        "search" => search(&args[1..]),
        "list" => list(),
        _ => Err("usage: asylum plugin <install <owner/repo> | search | list>".into()),
    }
}

fn install(args: &[String]) -> Result<(), String> {
    let spec = positionals(args)
        .first()
        .cloned()
        .ok_or("usage: asylum plugin install <owner/repo>[@ref]")?;
    let dir = plugin::default_dir();
    let dest = plugin::fetch(&spec, &dir)?;
    println!("installed {spec} -> {}", dest.display());
    Ok(())
}

fn list() -> Result<(), String> {
    let installed = plugin::load_dir(&plugin::default_dir());
    if installed.plugins.is_empty() {
        println!("no plugins installed ({})", plugin::default_dir().display());
    }
    for p in &installed.plugins {
        println!(
            "{:<22} {:<8} {}",
            p.id,
            p.version,
            p.description.as_deref().unwrap_or("")
        );
    }
    for d in &installed.diagnostics {
        eprintln!("! {}: {}", d.path.display(), d.message);
    }
    Ok(())
}

fn search(args: &[String]) -> Result<(), String> {
    let limit = flag(args, "--limit")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(30);
    let (program, argv) = plugin::discover_command(limit);
    let out = Command::new(&program).args(&argv).output().map_err(|e| {
        format!("could not run {program}: {e} (install the GitHub CLI to search plugins)")
    })?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(serde_json::Value::Array(items)) if !items.is_empty() => {
            for it in items {
                println!(
                    "{}  ★{}\n    {}\n    {}",
                    it["nameWithOwner"].as_str().unwrap_or("?"),
                    it["stargazersCount"].as_i64().unwrap_or(0),
                    it["description"].as_str().unwrap_or(""),
                    it["url"].as_str().unwrap_or("")
                );
            }
        }
        _ => println!("no plugins found for topic `{}`", plugin::TOPIC),
    }
    Ok(())
}
