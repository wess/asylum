//! Provider profiles: where Bots' models come from. An HTTP profile is any
//! chat-completions endpoint (xAI, OpenAI/ChatGPT, Ollama local or cloud, a
//! LiteLLM proxy, a custom gateway); a process profile wraps a coding-agent
//! CLI (Claude Code, Codex) fed the transcript on stdin. Profiles store
//! where a key lives, never the key.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
  #[default]
  Http,
  Process,
}

/// How a process provider reports its answer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Output {
  /// stdout is the reply.
  #[default]
  Text,
  /// stdout is a JSON object whose `result` is the reply.
  JsonResult,
  /// one JSON object per line, as `claude -p --output-format stream-json`.
  StreamJson,
}

/// Where a provider's key comes from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum Credential {
  #[default]
  None,
  /// An environment variable, by name.
  Env { var: String },
  /// A secret in Synapse's vault (e.g. "apis.OpenRouter"), exported as `var`.
  Synapse { secret: String, var: String },
  /// A token typed once and kept in the OS keychain under this account.
  Keychain { account: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
  pub name: String,
  pub preset: String,
  pub kind: Kind,
  pub endpoint: String,
  pub credential: Credential,
  pub command: String,
  pub args: Vec<String>,
  pub output: Output,
  /// Models known for this profile (discovered from /models, or listed).
  pub models: Vec<String>,
}

impl Profile {
  pub fn is_process(&self) -> bool {
    self.kind == Kind::Process
  }
}
