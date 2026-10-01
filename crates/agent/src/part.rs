//! What a message carries besides its text. Stored as JSON in
//! `messages.parts`; the UI renders each as a card.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
  /// A file the user attached (copied into the workspace).
  Attachment { name: String, path: String, mime: String, size: u64 },
  /// One tool call and its outcome.
  Tool { id: String, name: String, args: Value, result: String, status: String },
  /// Model reasoning, shown collapsed.
  Reasoning { text: String },
  /// An approval card; state lives in the approvals table.
  Approval { id: String },
  /// A question the Bot needs answered, with optional quick replies.
  Question { text: String, options: Vec<String> },
  /// A secure secret request: the value goes to the keychain, never the Bot.
  SecretRequest { name: String, description: String, status: String, fill: Option<u32> },
  /// "Action needed" on the computer: take over or skip.
  Takeover { reason: String, status: String },
  /// A form the Bot wants filled (one per step), e.g. a login or address.
  Form { title: String, fields: Vec<Field>, status: String, values: Value },
  /// A draft to approve before sending: email or Slack.
  Draft { kind: String, to: Vec<String>, subject: String, body: String, status: String, channel: String },
  /// A file the Bot created or changed on the computer.
  File { path: String, name: String, mime: String },
  /// An image (generated or captured).
  Image { path: String, caption: String },
  /// A link preview.
  Link { url: String, title: String },
  /// A structured reply: card, table, board, or chart.
  Card { kind: String, title: String, data: Value },
  /// A message from another Bot (a handoff) or to one.
  Handoff { from: String, to: String, direction: String },
  /// A routine was created, changed, run, or tested.
  Routine { id: String, name: String, event: String },
  /// A voice chat that ended, with its transcript.
  VoiceChat { seconds: u64, transcript: String },
  /// An audio message from the Bot.
  VoiceMemo { path: String, transcript: String },
  /// A skill drafted from a Teach-a-task demo, awaiting review.
  SkillDraft { name: String, description: String, instructions: String, status: String },
  /// Connect a plugin account.
  Connect { plugin: String, status: String },
  /// A run failed.
  Error { text: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Field {
  pub name: String,
  pub label: String,
  #[serde(default)]
  pub kind: String,
  #[serde(default)]
  pub required: bool,
}

pub fn to_values(parts: &[Part]) -> Vec<Value> {
  parts.iter().filter_map(|p| serde_json::to_value(p).ok()).collect()
}

pub fn from_values(values: &[Value]) -> Vec<Part> {
  values.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect()
}

pub fn parse(json: &str) -> Vec<Part> {
  let values: Vec<Value> = serde_json::from_str(json).unwrap_or_default();
  from_values(&values)
}

#[cfg(test)]
#[path = "../tests/part.rs"]
mod tests;
