//! Bot, group, section, routine, and skill management from the UI.

use crate::event::Event;
use crate::runtime::Runtime;
use anyhow::{bail, Result};
use store::bots::{self, Profile};
use store::{chats, routines, skills, Bot, Chat};

pub const DEFAULT_NAME: &str = "New Bot";

pub async fn create(rt: &Runtime, name: Option<&str>) -> Result<(Bot, Chat)> {
  let pool = &rt.pool;
  if bots::count(pool).await? + chats::groups(pool).await?.len() as i64 >= crate::turn::ROSTER_LIMIT {
    bail!("You have {} Bots and group chats. Delete one to make room.", crate::turn::ROSTER_LIMIT);
  }
  let base = name.map(str::trim).filter(|n| !n.is_empty()).unwrap_or(DEFAULT_NAME);
  let name = bots::unique_name(pool, base).await?;
  let b = bots::create(pool, &Profile { name, color: palette(bots::count(pool).await? as usize), ..Default::default() }).await?;
  let c = chats::direct(pool, &b.id).await?;
  if bots::primary(pool).await?.is_none() {
    bots::set_primary(pool, &b.id).await?;
  }
  rt.emit(Event::BotsChanged);
  Ok((b, c))
}

pub fn palette(i: usize) -> String {
  const C: [&str; 8] = ["#7c5cff", "#ff6b6b", "#1fb6ff", "#13ce66", "#ffb020", "#ff49db", "#00c2a8", "#8e44ad"];
  C[i % C.len()].into()
}

pub async fn update(rt: &Runtime, id: &str, p: &Profile) -> Result<Bot> {
  if p.name.trim().is_empty() {
    bail!("A Bot needs a name.");
  }
  let b = bots::update(&rt.pool, id, p).await?;
  rt.emit(Event::BotsChanged);
  Ok(b)
}

pub async fn rename(rt: &Runtime, id: &str, name: &str) -> Result<()> {
  let mut p = bots::get(&rt.pool, id).await?.profile();
  p.name = name.trim().to_string();
  update(rt, id, &p).await.map(|_| ())
}

/// A copy named "<name> copy" with the profile, settings, enabled skills,
/// routines, and avatar — no history, memory, or attachments.
pub async fn duplicate(rt: &Runtime, id: &str) -> Result<(Bot, Chat)> {
  let pool = &rt.pool;
  let src = bots::get(pool, id).await?;
  let mut p = src.profile();
  p.name = bots::unique_name(pool, &format!("{} copy", src.name)).await?;
  let b = bots::create(pool, &p).await?;
  bots::set_notifications(pool, &b.id, src.notifications).await?;
  for s in skills::enabled(pool, id).await? {
    skills::enable(pool, &b.id, &s.id, true).await?;
  }
  for r in routines::for_bot(pool, id).await? {
    let next = if r.active { schedule::next_run(&r.trigger, &r.schedule, store::now(), rt.tz()).ok().flatten() } else { None };
    let copy = routines::create(pool, routines::New {
      bot: b.id.clone(),
      name: r.name.clone(),
      instruction: r.instruction.clone(),
      trigger: r.trigger.clone(),
      schedule: r.schedule.clone(),
      filter: r.filter(),
      next_run: next,
    })
    .await?;
    if !r.active {
      routines::set_active(pool, &copy.id, false, None).await?;
    }
  }
  let c = chats::direct(pool, &b.id).await?;
  rt.emit(Event::BotsChanged);
  Ok((b, c))
}

/// Delete a Bot: its profile, conversations, and routines. Files on the
/// computer and browser sign-ins stay.
pub async fn delete(rt: &Runtime, id: &str) -> Result<()> {
  if store::bots::get(&rt.pool, id).await?.required {
    anyhow::bail!("Required by your admin.");
  }
  crate::turn::stop(rt, id);
  rt.browser.close_screen(id).await;
  for s in store::secrets::list(&rt.pool, id).await? {
    let _ = config::secret::remove(&s.key());
  }
  bots::delete(&rt.pool, id).await?;
  if bots::primary(&rt.pool).await?.is_none() {
    if let Some(first) = bots::list(&rt.pool).await?.into_iter().find(|b| b.kind != bots::SYSTEM) {
      bots::set_primary(&rt.pool, &first.id).await?;
    }
  }
  rt.emit(Event::BotsChanged);
  rt.emit(Event::ChatsChanged);
  Ok(())
}

pub async fn create_group(rt: &Runtime, ids: &[String]) -> Result<Chat> {
  if !(2..=6).contains(&ids.len()) {
    bail!("Pick 2 to 6 Bots for a group chat.");
  }
  let title = crate::turn::group_name(rt, ids).await;
  let c = chats::create_group(&rt.pool, &title, ids).await?;
  rt.emit(Event::ChatsChanged);
  Ok(c)
}

pub async fn set_members(rt: &Runtime, chat: &str, ids: &[String]) -> Result<()> {
  if !(2..=6).contains(&ids.len()) {
    bail!("A group chat has 2 to 6 Bots.");
  }
  let current = chats::members(&rt.pool, chat).await?;
  for id in &current {
    if !ids.contains(id) {
      chats::remove_member(&rt.pool, chat, id).await?;
    }
  }
  for id in ids {
    chats::add_member(&rt.pool, chat, id).await?;
  }
  rt.emit(Event::ChatsChanged);
  Ok(())
}

pub async fn set_routine_active(rt: &Runtime, id: &str, on: bool) -> Result<()> {
  let r = routines::get(&rt.pool, id).await?;
  let next = if on { schedule::next_run(&r.trigger, &r.schedule, store::now(), rt.tz())? } else { None };
  routines::set_active(&rt.pool, id, on, next).await?;
  rt.emit(Event::Routines { bot: r.bot_id });
  Ok(())
}

pub async fn test_routine(rt: &Runtime, id: &str) -> Result<()> {
  let r = routines::get(&rt.pool, id).await?;
  let chat = chats::direct(&rt.pool, &r.bot_id).await?;
  crate::turn::enqueue(rt, crate::Job { bot: r.bot_id.clone(), chat: chat.id, origin: crate::Origin::Test, routine: Some(r.id), note: String::new() });
  Ok(())
}

pub async fn delete_routine(rt: &Runtime, id: &str) -> Result<()> {
  let r = routines::get(&rt.pool, id).await?;
  routines::delete(&rt.pool, id).await?;
  rt.emit(Event::Routines { bot: r.bot_id });
  Ok(())
}

/// Add a Marketplace skill pack to the library.
pub async fn add_packaged_skill(rt: &Runtime, name: &str) -> Result<store::Skill> {
  let pack = crate::catalog::SKILLS.iter().find(|s| s.name == name).ok_or_else(|| anyhow::anyhow!("unknown skill"))?;
  let s = skills::save(&rt.pool, pack.name, pack.description, pack.instructions, skills::PACKAGED).await?;
  rt.emit(Event::Skills);
  Ok(s)
}

/// The Disk Saver system Bot, created on first need.
pub async fn disk_saver(rt: &Runtime) -> Result<(Bot, Chat)> {
  let pool = &rt.pool;
  if let Some(b) = bots::by_kind(pool, bots::SYSTEM).await?.into_iter().find(|b| b.name == "Disk Saver") {
    let c = chats::direct(pool, &b.id).await?;
    return Ok((b, c));
  }
  let b = bots::create_kind(pool, &Profile {
    name: "Disk Saver".into(),
    label: "Frees up space on the computer".into(),
    description: "Audit disk use on the shared computer (workspace, caches, downloads, node_modules, virtualenvs, build output). \
Propose what to clean with sizes. Delete nothing without the user's explicit confirmation of each item.".into(),
    avatar: "🧹".into(),
    color: "#00c2a8".into(),
  }, bots::SYSTEM, None).await?;
  let c = chats::direct(pool, &b.id).await?;
  rt.emit(Event::BotsChanged);
  Ok((b, c))
}
