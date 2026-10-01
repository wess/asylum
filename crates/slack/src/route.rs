//! Which Slack events a Team Bot answers, and which conversation each one
//! belongs to: one per DM, and one per thread in channels and group DMs.

use serde::Deserialize;
use std::collections::HashSet;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Event {
  #[serde(rename = "type", default)]
  pub kind: String,
  #[serde(default)]
  pub channel: String,
  #[serde(default)]
  pub channel_type: String,
  #[serde(default)]
  pub user: String,
  #[serde(default)]
  pub text: String,
  #[serde(default)]
  pub ts: String,
  pub thread_ts: Option<String>,
  pub bot_id: Option<String>,
  pub subtype: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
  Ignore,
  /// Answer in `conversation` (a stable key), replying in `thread` when set.
  Answer { conversation: String, thread: Option<String>, follow: bool },
}

/// `me` is the Bot's Slack user ID; `followed` holds "channel:thread_ts"
/// threads the Bot was mentioned in.
pub fn decide(e: &Event, me: &str, followed: &HashSet<String>) -> Decision {
  // Our own messages, other bots, edits, and joins are never answered.
  if e.bot_id.is_some() || e.user == me || e.user.is_empty() || e.subtype.is_some() {
    return Decision::Ignore;
  }
  if e.kind == "message" && e.channel_type == "im" {
    return Decision::Answer { conversation: format!("im:{}", e.channel), thread: None, follow: false };
  }
  let root = e.thread_ts.clone().unwrap_or_else(|| e.ts.clone());
  let key = format!("{}:{root}", e.channel);
  let mentioned = e.kind == "app_mention" || e.text.contains(&format!("<@{me}>"));
  if mentioned {
    return Decision::Answer { conversation: key, thread: Some(root), follow: true };
  }
  if e.kind == "message" && e.thread_ts.is_some() && followed.contains(&key) {
    return Decision::Answer { conversation: key, thread: Some(root), follow: false };
  }
  Decision::Ignore
}

/// Message text without the leading mention of the Bot.
pub fn clean(text: &str, me: &str) -> String {
  text.replace(&format!("<@{me}>"), "").trim().to_string()
}

#[cfg(test)]
#[path = "../tests/route.rs"]
mod tests;
