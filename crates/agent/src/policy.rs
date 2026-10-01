//! Applying the admin policy (`config::policy`): read it, then bring local
//! state in line — locked rules, required plugins, and default Team Bots.
//! Disabled plugins, the local-execution ceiling, forced Auto-review, and
//! template limits are checked where they apply.

use crate::event::Event;
use crate::runtime::Runtime;
use anyhow::Result;
use config::Policy;

/// Reload the policy from `path` and apply it. A broken file keeps the last
/// good policy and is reported.
pub async fn apply(rt: &Runtime, path: &std::path::Path) -> Result<()> {
  let p = match config::policy::load(path) {
    Ok(p) => {
      set_error(rt, None);
      p
    }
    Err(e) => {
      set_error(rt, Some(e));
      return Ok(());
    }
  };
  let changed = rt.policy() != p;
  if let Ok(mut w) = rt.policy.write() {
    *w = p.clone();
  }
  if let Ok(mut n) = rt.network.write() {
    *n = p.network.clone();
  }
  let limited = !matches!(p.network.mode.as_str(), "" | "open");
  if rt.browser.set_proxy(limited.then_some(rt.proxy)) {
    // Relaunch on next use with (or without) the egress proxy.
    rt.browser.shutdown().await;
  }
  sync(rt, &p).await?;
  if changed {
    crate::audit::change(rt, "admin", "policy.applied", &p.organization, &serde_json::to_string(&p).unwrap_or_default()).await;
    rt.emit(Event::Plugins);
    rt.emit(Event::BotsChanged);
  }
  Ok(())
}

fn set_error(rt: &Runtime, e: Option<String>) {
  if let Ok(mut w) = rt.policy_error.write() {
    *w = e;
  }
}

async fn sync(rt: &Runtime, p: &Policy) -> Result<()> {
  let rules: Vec<(String, String)> = p
    .rules
    .iter()
    .filter(|r| (r.kind == store::rules::ASK || r.kind == store::rules::ALLOW) && !r.text.trim().is_empty())
    .map(|r| (r.kind.clone(), r.text.clone()))
    .collect();
  store::rules::sync_locked(&rt.pool, &rules).await?;
  for id in &p.required_plugins {
    if store::plugins::by_catalog(&rt.pool, id).await?.is_none() {
      let _ = crate::api::connect::add(rt, id).await;
    }
  }
  // link -> Bot id for Team Bots the policy added.
  let joined: std::collections::BTreeMap<String, String> =
    store::state::get(&rt.pool, "policy_team_bots").await?.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
  let mut now = std::collections::BTreeMap::new();
  for link in &p.team_bots {
    let id = match joined.get(link) {
      Some(id) if store::bots::get(&rt.pool, id).await.is_ok() => Some(id.clone()),
      _ => crate::api::team::join(rt, link).await.ok().map(|(b, _)| b.id),
    };
    if let Some(id) = id {
      store::bots::set_required(&rt.pool, &id, true).await?;
      now.insert(link.clone(), id);
    }
  }
  // Dropped from the policy: no longer required (the Bot stays).
  for (link, id) in &joined {
    if !now.contains_key(link) {
      let _ = store::bots::set_required(&rt.pool, id, false).await;
    }
  }
  if now != joined {
    store::state::set(&rt.pool, "policy_team_bots", &serde_json::to_string(&now)?).await?;
  }
  Ok(())
}

/// Whether a plugin is required (so it can't be removed).
pub fn required(rt: &Runtime, catalog: &str) -> bool {
  rt.policy().required_plugins.iter().any(|r| r.eq_ignore_ascii_case(catalog))
}

#[cfg(test)]
#[path = "../tests/policy.rs"]
mod tests;
