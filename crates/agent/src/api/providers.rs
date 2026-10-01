//! Connecting a provider with a pasted key: no environment variables or
//! vaults needed. The key is checked against the provider first, then kept
//! in the macOS Keychain; the profile only records where it lives.

use crate::runtime::Runtime;
use anyhow::{bail, Result};
use config::{Credential, Profile};

/// Presets that take an API key, with the label people know them by.
pub const KEYED: [(&str, &str); 6] = [
  ("lite-llm", "LiteLLM gateway"),
  ("openai", "OpenAI"),
  ("anthropic", "Anthropic"),
  ("xai", "xAI"),
  ("ollama-cloud", "Ollama Cloud"),
  ("custom", "OpenAI-compatible"),
];

/// Whether a preset needs its address typed in (no fixed public endpoint).
pub fn needs_endpoint(preset: &str) -> bool {
  matches!(preset, "lite-llm" | "custom")
}

/// The profile a pasted key will live under: the preset's own Keychain
/// entry when it has one (xAI), else one named for the profile.
pub fn keyed_profile(preset: &str, name: &str, endpoint: &str) -> Profile {
  let mut p = provider::preset::build(preset, name);
  let endpoint = endpoint.trim().trim_end_matches('/');
  if !endpoint.is_empty() {
    p.endpoint = endpoint.to_string();
  }
  if !matches!(p.credential, Credential::Keychain { .. }) {
    p.credential = Credential::Keychain { account: provider::credential::account(name) };
  }
  p
}

/// Check `key` against the provider, save it, and add (or update) the
/// profile with the models it offers. The first provider becomes the default.
pub async fn connect(rt: &Runtime, preset: &str, name: &str, endpoint: &str, key: &str) -> Result<Profile> {
  let key = key.trim();
  if key.is_empty() {
    bail!("Paste the API key first.");
  }
  let mut p = keyed_profile(preset, name, endpoint);
  if p.endpoint.trim().is_empty() {
    bail!("Enter the provider's address, e.g. https://litellm.example.com/v1");
  }
  if !p.endpoint.starts_with("http://") && !p.endpoint.starts_with("https://") {
    bail!("The address should start with https://");
  }
  // Refuse a bad key now, not at an Agent's first turn.
  let models = if preset == "lite-llm" {
    provider::litellm::check(&p.endpoint, key).await?;
    provider::litellm::models(&p.endpoint, key, None).await.unwrap_or_default()
  } else {
    let client = chat::Client::new(key, Some(p.endpoint.clone())).retries(1);
    match client.models().await {
      Ok(list) => {
        let mut ids: Vec<String> = list.into_iter().map(|m| m.id).collect();
        ids.sort();
        ids
      }
      Err(e) => bail!("That key didn't work: {e}"),
    }
  };
  if let Credential::Keychain { account } = &p.credential {
    config::secret::set(account, key)?;
  }
  p.models = models;
  let mut s = rt.settings();
  let first = s.providers.is_empty() || s.provider.is_empty();
  s.providers.retain(|x| x.name != p.name);
  s.providers.push(p.clone());
  if first {
    s.provider = p.name.clone();
    s.model = p.models.first().cloned().unwrap_or_default();
  }
  config::save(&config::settings_path(), &s)?;
  rt.set_settings(s);
  crate::audit::change(rt, "user", "provider.connected", &p.name, preset).await;
  Ok(p)
}

#[cfg(test)]
#[path = "../../tests/api/providers.rs"]
mod tests;
