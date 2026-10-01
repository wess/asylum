//! Ready-made profiles, following ainz's presets.

use config::{Credential, Kind, Output, Profile};

pub const IDS: [(&str, &str); 9] = [
  ("xai", "xAI (Grok)"),
  ("openai", "OpenAI / ChatGPT"),
  ("anthropic", "Anthropic (Claude)"),
  ("lite-llm", "LiteLLM proxy"),
  ("ollama", "Ollama (local)"),
  ("ollama-cloud", "Ollama Cloud"),
  ("claude-code", "Claude Code (CLI)"),
  ("codex", "Codex (CLI)"),
  ("custom", "Custom endpoint"),
];

fn http(name: &str, preset: &str, endpoint: &str, credential: Credential) -> Profile {
  Profile { name: name.into(), preset: preset.into(), kind: Kind::Http, endpoint: endpoint.into(), credential, ..Default::default() }
}

fn process(name: &str, preset: &str, command: &str, args: &[&str], output: Output) -> Profile {
  Profile {
    name: name.into(),
    preset: preset.into(),
    kind: Kind::Process,
    command: command.into(),
    args: args.iter().map(|s| s.to_string()).collect(),
    output,
    models: crate::known::for_preset(preset),
    ..Default::default()
  }
}

pub fn build(preset: &str, name: &str) -> Profile {
  let env = |v: &str| Credential::Env { var: v.into() };
  let key = Credential::Keychain { account: crate::credential::account(name) };
  match preset {
    "xai" => http(name, preset, "https://api.x.ai/v1", Credential::Keychain { account: config::secret::XAI_KEY.into() }),
    "openai" => http(name, preset, "https://api.openai.com/v1", env("OPENAI_API_KEY")),
    // Anthropic's OpenAI-compatible endpoint: Claude models with our tools.
    "anthropic" => http(name, preset, "https://api.anthropic.com/v1", env("ANTHROPIC_API_KEY")),
    "lite-llm" => http(name, preset, "http://127.0.0.1:4000/v1", env("LITELLM_API_KEY")),
    "ollama" => http(name, preset, "http://127.0.0.1:11434/v1", Credential::None),
    "ollama-cloud" => http(name, preset, "https://api.ollama.com/v1", env("OLLAMA_API_KEY")),
    "claude-code" => process(
      name,
      preset,
      "claude",
      &["-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages", "--no-session-persistence", "--model", "{model}", "--permission-mode", "acceptEdits"],
      Output::StreamJson,
    ),
    "codex" => process(
      name,
      preset,
      "codex",
      &["exec", "--ephemeral", "--color", "never", "--sandbox", "workspace-write", "-C", "{workspace}", "--model", "{model}", "-"],
      Output::Text,
    ),
    _ => http(name, "custom", "", key),
  }
}

/// The xAI profile a fresh install starts with.
pub fn default_profile() -> Profile {
  build("xai", "xai")
}

#[cfg(test)]
#[path = "../tests/preset.rs"]
mod tests;
