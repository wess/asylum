//! Admin policy: the local stand-in for Grok Bot's admin console. An admin
//! (or MDM) installs `/Library/Application Support/asylum/policy.json`,
//! which only administrators can write; users can read but not change it.
//! `ASYLUM_POLICY` overrides the path (for tests and staging).

use crate::settings::LocalExec;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Policy {
  /// Shown as "Managed by <organization>".
  pub organization: String,
  /// Marketplace ids or plugin names Bots may not use.
  pub disabled_plugins: Vec<String>,
  /// Marketplace ids added for everyone (they still need signing in).
  pub required_plugins: Vec<String>,
  /// Approval rules everyone gets, locked against edits.
  pub rules: Vec<Rule>,
  /// The most local execution may allow.
  pub local_exec: Option<LocalExec>,
  /// Force Auto-review on or off.
  pub auto_review: Option<bool>,
  /// The widest template sharing allowed: "public", "team", or "off".
  pub templates: Option<String>,
  /// Team Bot links every teammate gets added automatically.
  pub team_bots: Vec<String>,
  /// Set false to turn Team Bots off for everyone.
  pub team_bots_enabled: Option<bool>,
  /// Where the computer's browser and web tools may go.
  pub network: Network,
  /// Set false to cut network access for commands run on this Mac.
  pub local_egress: Option<bool>,
  /// Set false to turn off Cloud Agents (CLI coding-agent providers).
  pub cloud_agents: Option<bool>,
  /// Run on the computer when it starts (setup), and to verify it (check).
  pub setup_script: String,
  pub check_script: String,
  /// Delete a computer that hasn't been used in this many days (0: never).
  pub terminate_inactive_days: u32,
  /// OTLP/HTTP collector everyone's telemetry goes to.
  pub otel_endpoint: String,
}

/// Network Controls. `open`: anywhere. `default`: anywhere except
/// `blocked`. `allowlist`: only `allowed` (plus their subdomains).
/// `offline`: no outbound access at all.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Network {
  pub mode: String,
  pub allowed: Vec<String>,
  pub blocked: Vec<String>,
}

impl Network {
  /// Whether `host` may be reached.
  pub fn allows(&self, host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let hit = |list: &[String]| {
      list.iter().any(|d| {
        let d = d.trim().trim_start_matches("*.").to_ascii_lowercase();
        !d.is_empty() && (host == d || host.ends_with(&format!(".{d}")))
      })
    };
    match self.mode.as_str() {
      "offline" => false,
      "allowlist" => hit(&self.allowed),
      "default" => !hit(&self.blocked),
      _ => true,
    }
  }

  /// Whether `url`'s host may be reached. Local pages (about:, data:,
  /// file:) are always allowed; unparseable URLs are refused when limited.
  pub fn allows_url(&self, url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    if ["about:", "data:", "file:", "blob:", "chrome:"].iter().any(|s| lower.starts_with(s)) {
      return true;
    }
    match host_of(&lower) {
      Some(h) => self.allows(&h),
      None => self.mode.is_empty() || self.mode == "open",
    }
  }
}

fn host_of(url: &str) -> Option<String> {
  let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
  let authority = rest.split(['/', '?', '#']).next()?;
  let host = authority.rsplit('@').next()?;
  let host = if host.starts_with('[') { host.split(']').next()?.trim_start_matches('[') } else { host.split(':').next()? };
  (!host.is_empty()).then(|| host.to_string())
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
  /// "ask" or "allow".
  pub kind: String,
  pub text: String,
}

pub const SYSTEM: &str = "/Library/Application Support/asylum/policy.json";

pub fn path() -> PathBuf {
  std::env::var_os("ASYLUM_POLICY").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(SYSTEM))
}

/// The policy at `p`; none (default) when missing. A malformed file is an
/// error, so a broken policy is reported, not silently ignored.
pub fn load(p: &Path) -> Result<Policy, String> {
  match std::fs::read_to_string(p) {
    Ok(t) => serde_json::from_str(&crate::jsonc::strip(&t)).map_err(|e| format!("policy.json: {e}")),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Policy::default()),
    Err(e) => Err(e.to_string()),
  }
}

impl Policy {
  pub fn managed(&self) -> bool {
    *self != Policy::default()
  }

  /// `want`, lowered to the policy's ceiling (Never < Ask < Always).
  pub fn cap_local(&self, want: LocalExec) -> LocalExec {
    let rank = |l: LocalExec| match l {
      LocalExec::Never => 0,
      LocalExec::Ask => 1,
      LocalExec::Always => 2,
    };
    match self.local_exec {
      Some(max) if rank(want) > rank(max) => max,
      _ => want,
    }
  }

  pub fn cloud_agents_allowed(&self) -> bool {
    self.cloud_agents != Some(false)
  }

  pub fn local_egress_allowed(&self) -> bool {
    self.local_egress != Some(false)
  }

  pub fn team_bots_allowed(&self) -> bool {
    self.team_bots_enabled != Some(false)
  }

  pub fn auto_review(&self, want: bool) -> bool {
    self.auto_review.unwrap_or(want)
  }

  /// Whether a plugin (by Marketplace id or name) is disabled.
  pub fn blocks(&self, catalog: Option<&str>, name: &str) -> bool {
    self.disabled_plugins.iter().any(|d| catalog.is_some_and(|c| c.eq_ignore_ascii_case(d)) || name.eq_ignore_ascii_case(d))
  }

  /// Whether templates may be shared with `visibility` ("public"/"team").
  pub fn allows_template(&self, visibility: &str) -> bool {
    match self.templates.as_deref() {
      Some("off") => false,
      Some("team") => visibility == "team",
      _ => true,
    }
  }
}

#[cfg(test)]
#[path = "../tests/policy.rs"]
mod tests;
