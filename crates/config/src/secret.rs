//! Secrets in the OS keychain (macOS Keychain, Windows Credential Manager,
//! Secret Service on Linux), keyed by a name under the app's service.

use crate::APP;
use anyhow::Result;

pub const XAI_KEY: &str = "xai-api-key";

fn entry(name: &str) -> Result<keyring::Entry> {
  Ok(keyring::Entry::new(APP, name)?)
}

pub fn get(name: &str) -> Option<String> {
  entry(name).ok()?.get_password().ok().filter(|s| !s.is_empty())
}

pub fn set(name: &str, value: &str) -> Result<()> {
  if value.is_empty() {
    return remove(name);
  }
  Ok(entry(name)?.set_password(value)?)
}

pub fn remove(name: &str) -> Result<()> {
  match entry(name)?.delete_credential() {
    Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
    Err(e) => Err(e.into()),
  }
}

/// The xAI key: the keychain first, then `XAI_API_KEY` for headless runs.
pub fn xai_key() -> Option<String> {
  get(XAI_KEY).or_else(|| std::env::var("XAI_API_KEY").ok().filter(|s| !s.is_empty()))
}
