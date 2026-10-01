//! Slack for Team Bots. Each Team Bot gets its own Slack app (created from
//! `manifest`), connected over Socket Mode (`socket`) so nothing has to be
//! reachable from the internet. `route` decides which events the Bot
//! answers: every DM, and channel or group-DM messages that mention it —
//! after which it follows that thread. `web` posts the replies.

pub mod manifest;
pub mod route;
pub mod socket;
pub mod web;

pub const BASE: &str = "https://slack.com/api";
