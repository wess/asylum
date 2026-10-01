//! Resolving a profile's key. Only pointers are stored; values are read
//! fresh from the environment, Synapse's vault, or the keychain each time.

use anyhow::Result;
use config::Credential;

pub async fn resolve(c: &Credential) -> Result<Option<String>> {
  Ok(match c {
    Credential::None => None,
    Credential::Env { var } => env(var),
    Credential::Keychain { account } => config::secret::get(account),
    Credential::Synapse { var, .. } => synapse(var).await,
  })
}

/// A profile's key. For an environment credential, an exported variable
/// wins; otherwise a key typed into Settings and kept in the keychain under
/// this profile is used, so a shell or CI job can override what setup saved.
pub async fn resolve_for(p: &config::Profile) -> Result<Option<String>> {
  if let Credential::Env { var } = &p.credential {
    return Ok(env(var).or_else(|| config::secret::get(&account(&p.name))));
  }
  resolve(&p.credential).await
}

fn env(var: &str) -> Option<String> {
  std::env::var(var).ok().filter(|v| !v.trim().is_empty())
}

/// A secret scoped to another directory is a miss, not an error.
async fn synapse(var: &str) -> Option<String> {
  let out = tokio::process::Command::new("synapse").args(["run", "--", "printenv", var]).output().await.ok()?;
  if !out.status.success() {
    return None;
  }
  let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
  (!v.is_empty()).then_some(v)
}

/// The keychain account a typed token for this profile lives under.
pub fn account(profile: &str) -> String {
  format!("provider-{profile}")
}

/// Where a credential points, for display ("env LITELLM_API_KEY").
pub fn describe(c: &Credential) -> String {
  match c {
    Credential::None => "no key".into(),
    Credential::Env { var } => format!("env {var}"),
    Credential::Synapse { secret, .. } => format!("Synapse {secret}"),
    Credential::Keychain { .. } => "Keychain".into(),
  }
}
