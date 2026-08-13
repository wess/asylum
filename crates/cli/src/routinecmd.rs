//! `asylum routine` — show Asylum a workflow once, replay it thereafter.
//!
//! Recording instruments a shell rather than watching the screen. What is worth
//! replaying in a repository is the commands, and commands are exactly what a
//! shell already knows: `bash` reports each one through a `DEBUG` trap, `zsh`
//! through `preexec`. Both hooks fire *before* the command runs, so a routine
//! records what you asked for even when it failed — which is usually what you
//! meant to record, since a failing step is still part of the sequence.
//!
//! The alternative, replaying keystrokes and screenshots, breaks the moment a
//! window moves. A command does not care where the window is.

use std::io::Write;

use crate::{flag, help, positionals};

/// Commands that are noise in a recording rather than part of the workflow.
///
/// Leaving the shell is how you *stop* recording, so it is never a step; and a
/// routine that re-invokes the recorder would nest one inside itself.
fn is_noise(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return true;
    }
    if t.starts_with('#') {
        return true;
    }
    matches!(t, "exit" | "logout" | "clear" | "ls" | "pwd")
        || t.starts_with("asylum routine record")
        || t.starts_with("asylum routine stop")
}

/// The steps worth keeping from a raw capture.
///
/// Consecutive duplicates collapse: a command re-run because it failed the
/// first time is one step in the workflow, not two, and replaying it twice is
/// at best slow and at worst destructive.
pub fn clean_steps(raw: &str) -> Vec<String> {
    let mut steps: Vec<String> = Vec::new();
    for line in raw.lines() {
        let step = line.trim();
        if is_noise(step) {
            continue;
        }
        if steps.last().map(String::as_str) == Some(step) {
            continue;
        }
        steps.push(step.to_string());
    }
    steps
}

/// The rc fragment that makes a shell report each command to `log`.
///
/// Sources the user's own rc first: recording inside a shell without your
/// aliases, prompt or PATH would change what the commands mean, and a routine
/// recorded in a stranger's environment is not the one you demonstrated.
pub fn recorder_rc(shell: &str, log: &str, user_rc: &str) -> String {
    if shell.contains("zsh") {
        format!(
            "[ -f {user_rc} ] && source {user_rc}\n\
             preexec() {{ print -r -- \"$1\" >> {log} }}\n\
             print -P '%F{{yellow}}recording — exit the shell to save%f'\n"
        )
    } else {
        format!(
            "[ -f {user_rc} ] && source {user_rc}\n\
             trap 'echo \"$BASH_COMMAND\" >> {log}' DEBUG\n\
             echo 'recording — exit the shell to save'\n"
        )
    }
}

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

pub fn routine(args: &[String]) -> Result<(), String> {
    let rest = &args[1.min(args.len())..];
    match args.first().map(String::as_str).unwrap_or("list") {
        "list" => {
            let db = open()?;
            let p = project(&db, flag(rest, "--project"))?;
            let all = db.routines(p.id).map_err(|e| e.to_string())?;
            if all.is_empty() {
                println!("no routines yet (try `asylum routine record <name>`)");
            }
            for r in all {
                println!(
                    "{:<18} {:>3} steps  {}",
                    r.name,
                    r.step_list().len(),
                    r.description
                );
            }
            Ok(())
        }
        "show" => {
            let db = open()?;
            let p = project(&db, flag(rest, "--project"))?;
            let name = positionals(rest).first().cloned().ok_or_else(|| {
                format!("a routine name is required {}", help::hint(&["routine"]))
            })?;
            let r = db
                .routine(p.id, &name)
                .map_err(|_| format!("no routine called {name:?}"))?;
            for (i, step) in r.step_list().iter().enumerate() {
                println!("{:>3}. {step}", i + 1);
            }
            Ok(())
        }
        "record" => record(rest),
        "run" => run(rest),
        "rm" => {
            let db = open()?;
            let p = project(&db, flag(rest, "--project"))?;
            let name = positionals(rest).first().cloned().ok_or_else(|| {
                format!("a routine name is required {}", help::hint(&["routine"]))
            })?;
            db.delete_routine(p.id, &name).map_err(|e| e.to_string())?;
            println!("removed {name}");
            Ok(())
        }
        other => Err(format!(
            "unknown `asylum routine {other}` {}",
            help::hint(&["routine"])
        )),
    }
}

/// Record a workflow by working through it in an instrumented shell.
fn record(args: &[String]) -> Result<(), String> {
    let name = positionals(args).first().cloned().ok_or_else(|| {
        format!(
            "a routine name is required {}",
            help::hint(&["routine", "record"])
        )
    })?;
    let description = flag(args, "--about").unwrap_or("").to_string();
    let db = open()?;
    let p = project(&db, flag(args, "--project"))?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("asylum-routine-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let log = dir.join("steps");
    std::fs::write(&log, "").map_err(|e| e.to_string())?;

    let zsh = shell.contains("zsh");
    let user_rc = if zsh {
        format!("{home}/.zshrc")
    } else {
        format!("{home}/.bashrc")
    };
    let rc = recorder_rc(&shell, &log.to_string_lossy(), &user_rc);

    println!("recording `{name}` — work through it, then exit the shell to save.");
    let status = if zsh {
        // zsh has no --rcfile; ZDOTDIR is the supported way to point it at a
        // different .zshrc, so the recorder lives in its own directory.
        std::fs::write(dir.join(".zshrc"), &rc).map_err(|e| e.to_string())?;
        std::process::Command::new(&shell)
            .env("ZDOTDIR", &dir)
            .status()
    } else {
        let rcfile = dir.join("rc");
        std::fs::write(&rcfile, &rc).map_err(|e| e.to_string())?;
        std::process::Command::new(&shell)
            .arg("--rcfile")
            .arg(&rcfile)
            .status()
    }
    .map_err(|e| format!("could not start {shell}: {e}"))?;
    let _ = status;

    let raw = std::fs::read_to_string(&log).unwrap_or_default();
    let steps = clean_steps(&raw);
    let _ = std::fs::remove_dir_all(&dir);
    if steps.is_empty() {
        return Err("nothing was recorded, so no routine was saved".to_string());
    }
    db.save_routine(p.id, &name, &description, &steps, now())
        .map_err(|e| e.to_string())?;
    println!("saved `{name}` with {} steps", steps.len());
    for (i, step) in steps.iter().enumerate() {
        println!("{:>3}. {step}", i + 1);
    }
    Ok(())
}

/// Replay a routine, stopping at the first step that fails.
fn run(args: &[String]) -> Result<(), String> {
    let name = positionals(args).first().cloned().ok_or_else(|| {
        format!(
            "a routine name is required {}",
            help::hint(&["routine", "run"])
        )
    })?;
    let db = open()?;
    let p = project(&db, flag(args, "--project"))?;
    let r = db
        .routine(p.id, &name)
        .map_err(|_| format!("no routine called {name:?}"))?;
    let steps = r.step_list();
    if steps.is_empty() {
        return Err(format!("`{name}` has no steps"));
    }

    for (i, step) in steps.iter().enumerate() {
        println!("[{}/{}] {step}", i + 1, steps.len());
        let _ = std::io::stdout().flush();
        let status = std::process::Command::new("sh")
            .arg("-lc")
            .arg(step)
            .status()
            .map_err(|e| format!("could not run step {}: {e}", i + 1))?;
        // Stopping at the first failure, not carrying on. The steps after a
        // failed one were demonstrated on the assumption it succeeded, and
        // running them anyway is how a half-finished release gets published.
        if !status.success() {
            return Err(format!(
                "step {} failed: {step}\n{} step(s) not run",
                i + 1,
                steps.len() - i - 1
            ));
        }
    }
    db.mark_routine_run(r.id, now())
        .map_err(|e| e.to_string())?;
    println!("`{name}` finished");
    Ok(())
}

#[cfg(test)]
#[path = "../tests/routinecmd.rs"]
mod tests;
