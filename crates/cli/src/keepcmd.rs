//! `asylum keep` - manage the encrypted secrets keep from the shell.
//!
//! The keep holds the values the secrets proxy injects for agents. It is
//! unlocked with a passphrase from `ASYLUM_KEEP_PASSPHRASE`; values are stored
//! per scope (global, or `--project <id>`), encrypted at rest.
//!
//! ```text
//! asylum keep set <name> [--project <id>] [--value <v>]   # value from stdin if omitted
//! asylum keep rm  <name> [--project <id>]
//! asylum keep list [--project <id>]
//! ```

use std::io::Read;

use keep::{Keep, Scope};

pub fn keep(args: &[String]) -> Result<(), String> {
    let sub = args.first().map(String::as_str).unwrap_or("");
    let pass = std::env::var("ASYLUM_KEEP_PASSPHRASE")
        .map_err(|_| "set ASYLUM_KEEP_PASSPHRASE to unlock the keep".to_string())?;
    if pass.is_empty() {
        return Err("ASYLUM_KEEP_PASSPHRASE is empty".into());
    }
    let path = keep_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let rest = &args[1.min(args.len())..];
    let scope = match crate::flag(rest, "--project").and_then(|p| p.parse::<i64>().ok()) {
        Some(id) => Scope::Project(id),
        None => Scope::Global,
    };

    let mut store = if path.exists() {
        Keep::open(&path, &pass).map_err(|e| e.to_string())?
    } else {
        Keep::create(&pass).map_err(|e| e.to_string())?
    };

    match sub {
        "set" => {
            let name = crate::positionals(rest)
                .into_iter()
                .next()
                .ok_or("usage: asylum keep set <name> [--project <id>] [--value <v>]")?;
            let value = match crate::flag(rest, "--value") {
                Some(v) => v.to_string(),
                None => read_stdin_value()?,
            };
            store.set(&scope, &name, value.trim_end_matches('\n'));
            store.save(&path).map_err(|e| e.to_string())?;
            println!("set {name} in {}", scope.key());
        }
        "rm" | "remove" => {
            let name = crate::positionals(rest)
                .into_iter()
                .next()
                .ok_or("usage: asylum keep rm <name> [--project <id>]")?;
            if store.remove(&scope, &name) {
                store.save(&path).map_err(|e| e.to_string())?;
                println!("removed {name} from {}", scope.key());
            } else {
                return Err(format!("no such secret: {name}"));
            }
        }
        "list" | "ls" => {
            for name in store.names(&scope) {
                println!("{name}");
            }
        }
        _ => return Err("usage: asylum keep <set|rm|list> [--project <id>]".into()),
    }
    Ok(())
}

/// The keep path, alongside `settings.json`.
fn keep_path() -> Result<std::path::PathBuf, String> {
    config::default_path()
        .parent()
        .map(|dir| dir.join("keep.enc"))
        .ok_or_else(|| "could not resolve the keep path".to_string())
}

/// Read a secret value from stdin (so it stays off the command line).
fn read_stdin_value() -> Result<String, String> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| e.to_string())?;
    if buf.trim().is_empty() {
        return Err("no value provided (pass --value or pipe it on stdin)".into());
    }
    Ok(buf)
}
