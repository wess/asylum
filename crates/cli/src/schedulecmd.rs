//! `asylum schedule` — work that starts without you.
//!
//! Writes straight to the store rather than talking to a running app, so a
//! schedule can be created from a terminal, a dotfile, or a provisioning
//! script with the ADE closed. The app picks it up on its next tick; SQLite is
//! in WAL mode, so a second writer is ordinary rather than a conflict.

use crate::{flag, help, positionals};

/// Minutes, from the words people actually use.
fn cadence(text: &str) -> Option<i64> {
    let t = text.trim().to_lowercase();
    let (number, unit) = t.split_at(t.find(|c: char| c.is_alphabetic())?);
    let n: i64 = number.trim().parse().ok()?;
    let minutes = match unit.trim() {
        "m" | "min" | "mins" | "minute" | "minutes" => n,
        "h" | "hr" | "hrs" | "hour" | "hours" => n * 60,
        "d" | "day" | "days" => n * 1440,
        "w" | "week" | "weeks" => n * 10080,
        _ => return None,
    };
    (minutes > 0).then_some(minutes)
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

pub fn schedule(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str).unwrap_or("list") {
        "list" => {
            let db = open()?;
            let projects = db.projects().map_err(|e| e.to_string())?;
            let mut any = false;
            for project in projects {
                for s in db.schedules(project.id).map_err(|e| e.to_string())? {
                    any = true;
                    let agents = if s.agents.is_empty() {
                        "project defaults".to_string()
                    } else {
                        s.agents.clone()
                    };
                    println!(
                        "{:<4} {:<20} {:<16} every {:<6} [{}]{}",
                        s.id,
                        s.title,
                        project.name,
                        human(s.every_minutes),
                        agents,
                        if s.enabled { "" } else { "  (disabled)" }
                    );
                }
            }
            if !any {
                println!("no schedules yet (try `asylum schedule add --help`)");
            }
            Ok(())
        }
        "add" => {
            let rest = &args[1..];
            let title = flag(rest, "--title").unwrap_or("Scheduled run");
            let every = flag(rest, "--every").ok_or_else(|| {
                format!("--every is required {}", help::hint(&["schedule", "add"]))
            })?;
            let minutes = cadence(every)
                .ok_or_else(|| format!("could not read --every {every:?}; try 1h, 30m, 1d"))?;
            let agents = flag(rest, "--agents").unwrap_or("");
            let prompt = positionals(rest).join(" ");
            if prompt.trim().is_empty() {
                return Err(format!(
                    "a prompt is required {}",
                    help::hint(&["schedule", "add"])
                ));
            }
            let db = open()?;
            // The project is the one named, else the most recently opened —
            // which is what somebody typing this in a repo means.
            let projects = db.projects().map_err(|e| e.to_string())?;
            let project = match flag(rest, "--project") {
                Some(name) => projects
                    .iter()
                    .find(|p| p.name == name || p.path == name)
                    .ok_or_else(|| format!("no project called {name:?}"))?,
                None => projects
                    .first()
                    .ok_or("open a project in Asylum first, so there is something to schedule")?,
            };
            let at = now();
            // First fire one whole cadence away, not immediately: `asylum
            // schedule add --every 1d` at 2pm means "nightly from tomorrow",
            // and firing on creation would fan out a run somebody did not ask
            // for while they were still typing.
            let s = db
                .create_schedule(
                    store::schedule::NewSchedule {
                        project_id: project.id,
                        title,
                        prompt: &prompt,
                        agents,
                        every_minutes: minutes,
                        // One whole cadence away, not immediate.
                        first_at: at + minutes * 60,
                    },
                    at,
                )
                .map_err(|e| e.to_string())?;
            println!(
                "scheduled {} on {} every {} (first run in {})",
                s.id,
                project.name,
                human(minutes),
                human(minutes)
            );
            Ok(())
        }
        "rm" => with_id(args, |db, id| {
            db.delete_schedule(id).map_err(|e| e.to_string())
        }),
        "enable" => with_id(args, |db, id| {
            db.set_schedule_enabled(id, true).map_err(|e| e.to_string())
        }),
        "disable" => with_id(args, |db, id| {
            db.set_schedule_enabled(id, false)
                .map_err(|e| e.to_string())
        }),
        other => Err(format!(
            "unknown `asylum schedule {other}` {}",
            help::hint(&["schedule"])
        )),
    }
}

fn with_id(
    args: &[String],
    act: impl Fn(&store::Db, i64) -> Result<(), String>,
) -> Result<(), String> {
    let id: i64 = positionals(&args[1..])
        .first()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("a schedule id is required {}", help::hint(&["schedule"])))?;
    let db = open()?;
    act(&db, id)?;
    println!("ok");
    Ok(())
}

/// A cadence in the words it was probably typed in.
pub fn human(minutes: i64) -> String {
    match minutes {
        m if m % 10080 == 0 => format!("{}w", m / 10080),
        m if m % 1440 == 0 => format!("{}d", m / 1440),
        m if m % 60 == 0 => format!("{}h", m / 60),
        m => format!("{m}m"),
    }
}

#[cfg(test)]
#[path = "../tests/schedulecmd.rs"]
mod tests;
