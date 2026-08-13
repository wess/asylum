//! The delegation thread: what the agents said to each other.
//!
//! Asylum's control surface already lets a running agent spawn a helper, read a
//! sibling's work, run the project's checks and block until another run reaches
//! a state — which is the most capable thing in the app and, until now, the
//! least visible. It happens inside a worktree, and the board shows runs.
//!
//! Nothing new is recorded here. `control_spawn`, `control_check`,
//! `run_activity` and the run lifecycle events have been written all along;
//! this turns them into something a person can read in order, so delegation
//! looks like a conversation rather than runs appearing for no stated reason.
//!
//! Pure: events in, lines out. The store query and the rendering live
//! elsewhere, so the interesting part — which event means what, and who is
//! speaking — is testable without a database or a window.

use serde_json::Value;
use store::Event;

/// One line of the thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// Who acted. The run's agent when known, else the app itself.
    pub who: Who,
    /// What happened, in plain words and already past tense.
    pub what: String,
    /// The run this line belongs to, for selecting it from the thread.
    pub run_id: Option<i64>,
    pub at: i64,
}

/// Whose voice a line is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Who {
    /// A named run — the agent id, e.g. `claude-code`.
    Agent(String),
    /// Asylum itself: dispatching, merging, finishing.
    Ade,
}

impl Who {
    pub fn label(&self) -> &str {
        match self {
            Who::Agent(name) => name,
            Who::Ade => "Asylum",
        }
    }
}

/// A run's agent name, for putting a line in somebody's voice.
pub type AgentOf = dyn Fn(i64) -> Option<String>;

fn field(data: &str, key: &str) -> Option<String> {
    serde_json::from_str::<Value>(data)
        .ok()?
        .get(key)?
        .as_str()
        .map(str::to_string)
}

/// Turn one task's events into a readable thread.
///
/// Events that say nothing a person needs are dropped rather than rendered as
/// noise — a thread that reports every heartbeat is one nobody reads, and the
/// point of this screen is that the interesting moments stand out.
pub fn thread(events: &[Event], agent_of: &AgentOf) -> Vec<Line> {
    let mut lines = Vec::new();
    for event in events {
        let who = event
            .run_id
            .and_then(agent_of)
            .map(Who::Agent)
            .unwrap_or(Who::Ade);

        let what = match event.kind.as_str() {
            // The delegation itself: one agent asking for another.
            "control_spawn" => {
                let agent = field(&event.data, "agent").unwrap_or_else(|| "an agent".into());
                match field(&event.data, "prompt") {
                    Some(prompt) if !prompt.trim().is_empty() => {
                        format!("asked {agent} to {}", first_clause(&prompt))
                    }
                    _ => format!("asked {agent} for help"),
                }
            }
            "control_check" => "ran the project's checks".to_string(),
            // Activity is the signal worth surfacing: "blocked" is the whole
            // reason the semantic states exist.
            "run_activity" => match field(&event.data, "activity").as_deref() {
                Some("blocked") => "is blocked and waiting on you".to_string(),
                Some("done") => "says it is done".to_string(),
                Some(other) => format!("is {other}"),
                None => continue,
            },
            "worktree_created" => "started work in its own worktree".to_string(),
            "run_started" => "began".to_string(),
            "run_finished" => "finished".to_string(),
            "task_created" => "took the task".to_string(),
            "mcp_call" => match field(&event.data, "tool") {
                Some(tool) => format!("used {tool}"),
                None => continue,
            },
            // Anything else is machinery, not conversation.
            _ => continue,
        };

        lines.push(Line {
            who,
            what,
            run_id: event.run_id,
            at: event.created_at,
        });
    }
    lines
}

/// The first sentence or clause of a prompt, trimmed for a one-line summary.
///
/// A spawn prompt is often several paragraphs. Rendering all of it turns the
/// thread into the thing it was meant to summarise.
fn first_clause(prompt: &str) -> String {
    let flat = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut = flat
        .find(". ")
        .map(|i| i + 1)
        .unwrap_or(flat.len())
        .min(flat.len());
    let head: String = flat[..cut].chars().take(80).collect();
    let trimmed = head.trim_end_matches(['.', ' ']).to_string();
    if flat.len() > trimmed.len() {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}

#[cfg(test)]
#[path = "../tests/delegation.rs"]
mod tests;

use crate::state::Root;

impl Root {
    /// The selected task's delegation thread, newest last.
    ///
    /// Capped: a long-running task can accumulate thousands of events, and the
    /// thread is a summary — reading the whole log to render a panel would make
    /// opening a task slower the longer you have worked on it.
    pub fn delegation_thread(&self) -> Vec<Line> {
        let Some(task_id) = self.task_id else {
            return Vec::new();
        };
        let Ok(events) = self.db.events_for_task(task_id, 500) else {
            return Vec::new();
        };
        // Resolved once into a map rather than a query per line: a thread of a
        // few hundred lines would otherwise be a few hundred round trips.
        let agents: std::collections::HashMap<i64, String> = self
            .db
            .runs(task_id)
            .unwrap_or_default()
            .into_iter()
            .map(|run| (run.id, run.agent))
            .collect();
        let lookup = move |id: i64| agents.get(&id).cloned();
        thread(&events, &lookup)
    }
}
