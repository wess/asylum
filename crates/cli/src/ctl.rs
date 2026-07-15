//! `asylum control` and `asylum wait` - orchestrate the ADE from inside a
//! worktree. Reads the control-surface env vars the app injects
//! ([`control::ENV_URL`] et al.) and talks to the local control server.

use std::thread::sleep;
use std::time::{Duration, Instant};

use control::Client;
use serde_json::Value;

use crate::{flag, positionals};

/// `asylum control <sub>`.
pub fn control(args: &[String]) -> Result<(), String> {
    let sub = args.first().map(String::as_str).unwrap_or("");
    let rest = &args[1..];
    match sub {
        "skill" => {
            println!("{}", control::SKILL);
            Ok(())
        }
        "status" => status(),
        "read" => read(rest),
        "spawn" => spawn(rest),
        "activity" => activity(rest),
        "check" => check(),
        _ => Err("usage: asylum control <status|read|spawn|activity|check|skill>".into()),
    }
}

/// `asylum wait run <id> [--status s] [--activity a] [--timeout secs]`.
pub fn wait(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) != Some("run") {
        return Err(
            "usage: asylum wait run <id> [--status <s>] [--activity <a>] [--timeout <secs>]".into(),
        );
    }
    let rest = &args[1..];
    let id = positionals(rest)
        .first()
        .cloned()
        .ok_or("usage: asylum wait run <id> ...")?;
    let want_status = flag(rest, "--status").map(String::from);
    let want_activity = flag(rest, "--activity").map(String::from);
    if want_status.is_none() && want_activity.is_none() {
        return Err("wait needs --status or --activity".into());
    }
    let timeout = flag(rest, "--timeout")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(600);

    let c = client()?;
    let deadline = Instant::now() + Duration::from_secs(timeout);
    loop {
        if let Ok((200, body)) = c.get(&format!("/control/runs/{id}")) {
            let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
            let status = v["status"].as_str().unwrap_or("");
            let act = v["activity"].as_str().unwrap_or("");
            let status_ok = want_status.as_deref().is_none_or(|w| w == status);
            let activity_ok = want_activity.as_deref().is_none_or(|w| w == act);
            if status_ok && activity_ok {
                println!(
                    "run {id}: {status} / {}",
                    if act.is_empty() { "-" } else { act }
                );
                return Ok(());
            }
            // A run that has ended will never reach a live activity.
            if want_status.is_none() && matches!(status, "succeeded" | "failed" | "cancelled") {
                return Err(format!(
                    "run {id} ended ({status}) before it was {:?}",
                    want_activity
                ));
            }
        }
        if Instant::now() >= deadline {
            return Err(format!("timed out after {timeout}s waiting for run {id}"));
        }
        sleep(Duration::from_millis(750));
    }
}

fn client() -> Result<Client, String> {
    Client::from_env()
        .ok_or_else(|| format!("not inside an Asylum worktree ({} unset)", control::ENV_URL))
}

fn require(key: &str) -> Result<String, String> {
    std::env::var(key).map_err(|_| format!("{key} is not set (are you inside an Asylum run?)"))
}

fn status() -> Result<(), String> {
    let c = client()?;
    let task = require(control::ENV_TASK)?;
    let me = std::env::var(control::ENV_RUN).unwrap_or_default();
    let (code, body) = c.get(&format!("/control/runs?task={task}"))?;
    if code != 200 {
        return Err(format!("control server returned {code}: {body}"));
    }
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    println!("task {task} (you are run {me})");
    if let Some(runs) = v["runs"].as_array() {
        for r in runs {
            let id = r["id"].as_i64().unwrap_or(0);
            let mark = if id.to_string() == me { "*" } else { " " };
            println!(
                "{mark} run {:<4} {:<14} {:<10} {}",
                id,
                r["agent"].as_str().unwrap_or("?"),
                r["status"].as_str().unwrap_or("?"),
                r["activity"].as_str().unwrap_or("-"),
            );
        }
    }
    Ok(())
}

fn read(args: &[String]) -> Result<(), String> {
    let id = positionals(args)
        .first()
        .cloned()
        .ok_or("usage: asylum control read <run-id>")?;
    let c = client()?;
    let (code, body) = c.get(&format!("/control/runs/{id}"))?;
    if code != 200 {
        return Err(format!("{code}: {body}"));
    }
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    println!("{}", v["output_tail"].as_str().unwrap_or(""));
    Ok(())
}

fn spawn(args: &[String]) -> Result<(), String> {
    let pos = positionals(args);
    let agent = pos
        .first()
        .ok_or("usage: asylum control spawn <agent> <prompt...>")?;
    let prompt = pos[1..].join(" ");
    let c = client()?;
    let task = require(control::ENV_TASK)?;
    let from_run = std::env::var(control::ENV_RUN)
        .ok()
        .and_then(|s| s.parse::<i64>().ok());
    let payload = serde_json::json!({ "agent": agent, "prompt": prompt, "from_run": from_run });
    let (code, resp) = c.post(
        &format!("/control/tasks/{task}/spawn"),
        &payload.to_string(),
    )?;
    if code != 200 {
        return Err(format!("{code}: {resp}"));
    }
    println!("queued {agent} on task {task}");
    Ok(())
}

fn activity(args: &[String]) -> Result<(), String> {
    let state = positionals(args)
        .first()
        .cloned()
        .ok_or("usage: asylum control activity <working|blocked|done|idle>")?;
    let c = client()?;
    let run = require(control::ENV_RUN)?;
    let payload = serde_json::json!({ "activity": state });
    let (code, resp) = c.post(
        &format!("/control/runs/{run}/activity"),
        &payload.to_string(),
    )?;
    if code != 200 {
        return Err(format!("{code}: {resp}"));
    }
    Ok(())
}

fn check() -> Result<(), String> {
    let c = client()?;
    let run = require(control::ENV_RUN)?;
    let (code, resp) = c.post(&format!("/control/runs/{run}/check"), "")?;
    if code != 200 {
        return Err(format!("{code}: {resp}"));
    }
    println!("queued checks for run {run}");
    Ok(())
}
