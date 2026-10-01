//! Bot templates: a portable copy of a Bot's identity, description, skills,
//! and routines. Never the computer, sign-ins, memory, or history. A link is
//! `asylum://template/<base64url json>`.

use crate::runtime::Runtime;
use anyhow::{anyhow, bail, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use store::{bots, routines, skills};

pub const SCHEME: &str = "asylum://template/";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Template {
  pub name: String,
  pub label: String,
  pub description: String,
  pub avatar: String,
  pub color: String,
  pub skills: Vec<SkillDef>,
  pub routines: Vec<RoutineDef>,
  #[serde(default)]
  pub visibility: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SkillDef {
  pub name: String,
  pub description: String,
  pub instructions: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RoutineDef {
  pub name: String,
  pub instruction: String,
  pub trigger: String,
  pub schedule: String,
  pub filter: serde_json::Value,
}

pub async fn build(rt: &Runtime, bot: &str) -> Result<Template> {
  let b = bots::get(&rt.pool, bot).await?;
  Ok(Template {
    name: b.name,
    label: b.label,
    description: b.description,
    // Uploaded images stay local; generated or character avatars travel.
    avatar: if b.avatar.starts_with('/') { String::new() } else { b.avatar },
    color: b.color,
    skills: skills::enabled(&rt.pool, bot)
      .await?
      .into_iter()
      .map(|s| SkillDef { name: s.name, description: s.description, instructions: s.instructions })
      .collect(),
    routines: routines::for_bot(&rt.pool, bot)
      .await?
      .into_iter()
      .map(|r| RoutineDef { filter: r.filter(), name: r.name, instruction: r.instruction, trigger: r.trigger, schedule: r.schedule })
      .collect(),
    visibility: String::new(),
  })
}

pub fn link(t: &Template) -> Result<String> {
  let json = serde_json::to_vec(t)?;
  Ok(format!("{SCHEME}{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)))
}

pub fn parse(link: &str) -> Result<Template> {
  let data = link.trim().strip_prefix(SCHEME).ok_or_else(|| anyhow!("not a Bot template link"))?;
  let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(data.trim_end_matches('='))?;
  let t: Template = serde_json::from_slice(&bytes)?;
  if t.name.trim().is_empty() {
    bail!("the template has no name");
  }
  Ok(t)
}

/// Words that suggest a template carries something that should not be
/// shared (keys, internal URLs, customer data).
pub fn warnings(t: &Template) -> Vec<String> {
  let mut text = format!("{} {}", t.description, t.label);
  for s in &t.skills {
    text.push_str(&s.instructions);
  }
  for r in &t.routines {
    text.push_str(&r.instruction);
  }
  let lower = text.to_lowercase();
  let mut out = Vec::new();
  if ["sk-", "xai-", "ghp_", "xoxb-", "api_key", "apikey", "secret", "password", "bearer "].iter().any(|k| lower.contains(k)) {
    out.push("It may contain a key, token, or password.".into());
  }
  if [".internal", "localhost", "127.0.0.1", "10.0.", "192.168.", ".corp", "intranet"].iter().any(|k| lower.contains(k)) {
    out.push("It may contain internal URLs.".into());
  }
  if lower.contains('@') && lower.contains(".com") {
    out.push("It may contain email addresses or customer data.".into());
  }
  out
}

/// Add a template as a new Bot on this account.
pub async fn install(rt: &Runtime, t: &Template) -> Result<store::Bot> {
  let pool = &rt.pool;
  let name = bots::unique_name(pool, &t.name).await?;
  let b = bots::create(pool, &bots::Profile {
    name,
    label: t.label.clone(),
    description: t.description.clone(),
    avatar: t.avatar.clone(),
    color: t.color.clone(),
  })
  .await?;
  for s in &t.skills {
    let sk = skills::save(pool, &s.name, &s.description, &s.instructions, skills::WRITTEN).await?;
    skills::enable(pool, &b.id, &sk.id, true).await?;
  }
  for r in &t.routines {
    let next = schedule::next_run(&r.trigger, &r.schedule, store::now(), rt.tz()).ok().flatten();
    routines::create(pool, routines::New {
      bot: b.id.clone(),
      name: r.name.clone(),
      instruction: r.instruction.clone(),
      trigger: r.trigger.clone(),
      schedule: r.schedule.clone(),
      filter: r.filter.clone(),
      next_run: next,
    })
    .await?;
  }
  store::chats::direct(pool, &b.id).await?;
  Ok(b)
}

#[cfg(test)]
#[path = "../tests/template.rs"]
mod tests;

/// Build, save, and link a template, within the admin's sharing limit.
pub async fn share(rt: &Runtime, bot: &str, visibility: &str) -> Result<String> {
  if !rt.policy().allows_template(visibility) {
    bail!(if visibility == "public" { "Your admin allows team-only templates." } else { "Your admin has turned off template sharing." });
  }
  let mut tpl = build(rt, bot).await?;
  tpl.visibility = visibility.to_string();
  let link = link(&tpl)?;
  store::templates::upsert(&rt.pool, bot, &tpl.name, visibility, &serde_json::to_string(&tpl)?).await?;
  Ok(link)
}
