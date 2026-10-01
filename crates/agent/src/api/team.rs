//! Team Bots. One Bot the owner sets up and publishes; each teammate chats
//! with it privately; it keeps team memory plus private notes per person.
//! Publishing is gated: a non-default name and a description (140
//! characters, in the Bot's voice) are required. Teammates join through a
//! team link, which carries the Bot's setup (not its computer or history).

use crate::event::Event;
use crate::runtime::Runtime;
use crate::template::{self, Template};
use anyhow::{bail, Result};
use store::bots::{self, Profile};
use store::{chats, memories, routines, Bot, Chat};

pub const DEFAULT_NAME: &str = "New team bot";
pub const MAX_DESCRIPTION: usize = 140;
pub const MAX_FILE_CHARS: usize = 256_000;
pub const FILE_TYPES: [&str; 7] = ["txt", "md", "markdown", "csv", "json", "yaml", "yml"];

pub const SETUP: &str = "Hi! I'm your new Team Bot. Tell me what your team needs from me, and I'll walk you through \
connecting plugins, adding secrets, uploading reference files, and saving skills. When I'm ready, publish me to your team.";

/// Error texts the UI turns into full screens.
pub const NOT_AVAILABLE: &str = "Team Bots Not Available";
pub const NOT_FOUND: &str = "Bot Not Found";

pub async fn create(rt: &Runtime) -> Result<(Bot, Chat)> {
  if !rt.policy().team_bots_allowed() {
    bail!("{NOT_AVAILABLE}");
  }
  let pool = &rt.pool;
  let name = bots::unique_name(pool, DEFAULT_NAME).await?;
  let owner = rt.settings().user_name;
  let b = bots::create_kind(pool, &Profile { name, color: "#1fb6ff".into(), ..Default::default() }, bots::TEAM, None).await?;
  sqlx_owner(rt, &b.id, &owner).await?;
  let c = chats::direct(pool, &b.id).await?;
  let m = store::messages::add(pool, store::messages::New {
    chat: &c.id,
    bot: Some(&b.id),
    role: store::messages::BOT,
    body: SETUP,
    parts: &[],
    status: store::messages::DONE,
    run: None,
    thread: None,
  })
  .await?;
  rt.emit(Event::Message { chat: c.id.clone(), message: m.id });
  rt.emit(Event::BotsChanged);
  Ok((bots::get(pool, &b.id).await?, c))
}

async fn sqlx_owner(rt: &Runtime, bot: &str, owner: &str) -> Result<()> {
  store::bots::set_owner(&rt.pool, bot, owner).await
}

pub fn ready(b: &Bot) -> Result<()> {
  if b.name.trim().is_empty() || b.name.starts_with(DEFAULT_NAME) {
    bail!("Give your Team Bot a name before publishing.");
  }
  if b.description.trim().is_empty() {
    bail!("Add a description before publishing.");
  }
  if b.description.chars().count() > MAX_DESCRIPTION {
    bail!("Keep the description to {MAX_DESCRIPTION} characters.");
  }
  Ok(())
}

/// How a personal Bot becomes a Team Bot.
pub enum Start {
  /// Copy the Bot, with the chosen memories as team memory.
  Copy { memories: Vec<String>, move_routines: bool },
  Fresh,
}

pub async fn convert(rt: &Runtime, bot: &str, start: Start) -> Result<(Bot, Chat)> {
  let pool = &rt.pool;
  let src = bots::get(pool, bot).await?;
  match start {
    Start::Fresh => {
      let (b, c) = create(rt).await?;
      let mut p = b.profile();
      p.name = bots::unique_name(pool, &src.name).await?;
      bots::update(pool, &b.id, &p).await?;
      Ok((bots::get(pool, &b.id).await?, c))
    }
    Start::Copy { memories: picked, move_routines } => {
      let (b, c) = crate::api::bots::duplicate(rt, bot).await?;
      bots::set_kind(pool, &b.id, bots::TEAM).await?;
      let mut p = src.profile();
      p.name = b.name.clone();
      p.description = src.description.chars().take(MAX_DESCRIPTION).collect();
      bots::update(pool, &b.id, &p).await?;
      memories::copy(pool, &picked, &b.id, memories::TEAM).await?;
      if move_routines {
        for r in routines::for_bot(pool, &b.id).await? {
          routines::delete(pool, &r.id).await?;
        }
        for r in routines::for_bot(pool, bot).await? {
          routines::reassign(pool, &r.id, &b.id).await?;
        }
      }
      sqlx_owner(rt, &b.id, &rt.settings().user_name).await?;
      rt.emit(Event::BotsChanged);
      Ok((bots::get(pool, &b.id).await?, c))
    }
  }
}

pub async fn publish(rt: &Runtime, bot: &str, on: bool) -> Result<String> {
  let b = bots::get(&rt.pool, bot).await?;
  if on {
    ready(&b)?;
  }
  bots::set_published(&rt.pool, bot, on).await?;
  rt.emit(Event::BotsChanged);
  link(rt, bot).await
}

/// The link teammates open to add this Team Bot.
pub async fn link(rt: &Runtime, bot: &str) -> Result<String> {
  let mut t: Template = template::build(rt, bot).await?;
  t.visibility = "team".into();
  template::link(&t)
}

/// Add a Team Bot from a teammate's link.
pub async fn join(rt: &Runtime, link: &str) -> Result<(Bot, Chat)> {
  if !rt.policy().team_bots_allowed() {
    bail!("{NOT_AVAILABLE}");
  }
  let t = template::parse(link).map_err(|_| anyhow::anyhow!("{NOT_FOUND}"))?;
  let b = template::install(rt, &t).await?;
  bots::set_kind(&rt.pool, &b.id, bots::TEAM).await?;
  bots::set_published(&rt.pool, &b.id, true).await?;
  let c = chats::direct(&rt.pool, &b.id).await?;
  rt.emit(Event::BotsChanged);
  Ok((bots::get(&rt.pool, &b.id).await?, c))
}

/// Upload a reference file to the Team Bot's files (in the workspace).
pub fn add_file(rt: &Runtime, bot: &Bot, path: &std::path::Path) -> Result<String> {
  let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
  let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
  if !FILE_TYPES.contains(&ext.as_str()) {
    bail!("Team files can be .txt, .md, .markdown, .csv, .json, .yaml, or .yml.");
  }
  let text = std::fs::read_to_string(path)?;
  if text.chars().count() > MAX_FILE_CHARS {
    bail!("Team files can be at most 256,000 characters.");
  }
  let rel = format!("team/{}/{}", crate::plugins::slug(&bot.name), name);
  computer::fs::write(&rt.computer.workspace(), &rel, &text)
}

pub fn files(rt: &Runtime, bot: &Bot) -> Vec<computer::fs::Entry> {
  let rel = format!("team/{}", crate::plugins::slug(&bot.name));
  computer::fs::list(&rt.computer.workspace(), &rel).unwrap_or_default()
}
