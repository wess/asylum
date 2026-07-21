//! Trigger dispatch: match an ADE event against installed plugins' `[[trigger]]`
//! hooks, and define the JSON payload a fired trigger receives.
//!
//! Pure and host-agnostic. This decides *which* enabled plugins fire for an
//! event (honoring each trigger's optional `when` filter) and shapes the
//! payload; the host (`app`) owns the side effects — posting a `notify` or
//! invoking a runtime off the UI thread. The trust gate lives here too: a
//! plugin the caller does not consider enabled never contributes a trigger, so
//! there is no path to fire a hook for a plugin the user has not turned on.

use serde::{Deserialize, Serialize};

use crate::{Plugin, Trigger};

/// The JSON payload handed to a fired trigger: the event name plus whatever ids
/// and paths are known at the event site. Absent fields are omitted from the
/// serialized object, so a plugin sees only what the event actually carried.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventPayload {
    /// The event name, one of [`TRIGGER_EVENTS`](crate::TRIGGER_EVENTS).
    pub event: String,
    /// The task this event belongs to, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<i64>,
    /// The run this event belongs to, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<i64>,
    /// The project's repository path, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The run's worktree path, when the event concerns one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    /// A coarse outcome token used both as data and as the value a trigger's
    /// `when` filter matches against (`success` / `failure` / `merged` / …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// A process exit code, when the event carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
}

impl EventPayload {
    /// A payload for `event` with no associated ids yet; fill it with the
    /// builder setters below.
    pub fn new(event: impl Into<String>) -> Self {
        Self {
            event: event.into(),
            ..Default::default()
        }
    }

    pub fn task(mut self, id: i64) -> Self {
        self.task = Some(id);
        self
    }

    pub fn run(mut self, id: i64) -> Self {
        self.run = Some(id);
        self
    }

    pub fn project(mut self, path: impl Into<String>) -> Self {
        self.project = Some(path.into());
        self
    }

    pub fn worktree(mut self, path: impl Into<String>) -> Self {
        self.worktree = Some(path.into());
        self
    }

    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    pub fn code(mut self, code: i32) -> Self {
        self.code = Some(code);
        self
    }
}

/// One plugin trigger that matched a dispatched event.
pub struct Fired<'a> {
    pub plugin: &'a Plugin,
    pub trigger: &'a Trigger,
}

/// Every trigger that should fire for `payload`: across the `plugins` the caller
/// considers enabled (`is_enabled(&plugin.id)` is true), each `[[trigger]]`
/// whose `on` equals the event and whose optional `when` filter admits the
/// payload's status. A disabled plugin contributes nothing — enabling is the
/// trust gate, enforced here.
pub fn fired<'a>(
    plugins: &'a [Plugin],
    is_enabled: impl Fn(&str) -> bool,
    payload: &EventPayload,
) -> Vec<Fired<'a>> {
    let mut out = Vec::new();
    for plugin in plugins {
        if !is_enabled(&plugin.id) {
            continue;
        }
        for trigger in &plugin.triggers {
            if trigger.on == payload.event
                && when_matches(trigger.when.as_deref(), payload.status.as_deref())
            {
                out.push(Fired { plugin, trigger });
            }
        }
    }
    out
}

/// Whether a trigger's optional `when` filter admits this payload status. No
/// filter (or a blank one) always matches. Otherwise the filter matches the
/// status case-insensitively, with `zero`/`nonzero` accepted as aliases for
/// `success`/`failure` so an exit-code vocabulary reads naturally.
fn when_matches(when: Option<&str>, status: Option<&str>) -> bool {
    let Some(when) = when.map(str::trim).filter(|w| !w.is_empty()) else {
        return true;
    };
    let status = status.unwrap_or("");
    when.eq_ignore_ascii_case(status)
        || (when.eq_ignore_ascii_case("nonzero") && status.eq_ignore_ascii_case("failure"))
        || (when.eq_ignore_ascii_case("zero") && status.eq_ignore_ascii_case("success"))
}

#[cfg(test)]
#[path = "../tests/dispatch.rs"]
mod tests;
