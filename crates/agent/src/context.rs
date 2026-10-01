//! Everything a turn needs, loaded once at its start.

use crate::runtime::Runtime;
use anyhow::Result;
use store::{Bot, Chat, Memory, Message, Routine, Skill};

pub struct Context {
  pub bot: Bot,
  pub chat: Chat,
  /// Other members of a group chat (empty for a 1:1).
  pub members: Vec<Bot>,
  pub roster: Vec<Bot>,
  pub memory: Vec<Memory>,
  pub skills: Vec<Skill>,
  pub routines: Vec<Routine>,
  pub secrets: Vec<String>,
  pub history: Vec<Message>,
  pub plugins: Vec<String>,
  pub user: String,
  pub now: String,
  pub zone: String,
  pub workspace: String,
  pub language: String,
  /// "<provider> · <model>" this turn runs on.
  pub model: String,
}

pub const HISTORY: i64 = 60;

pub async fn load(rt: &Runtime, bot: &str, chat: &str) -> Result<Context> {
  let pool = &rt.pool;
  let bot = store::bots::get(pool, bot).await?;
  let chat = store::chats::get(pool, chat).await?;
  let roster = store::bots::list(pool).await?;
  let members = if chat.is_group() {
    let ids = store::chats::members(pool, &chat.id).await?;
    roster.iter().filter(|b| ids.contains(&b.id) && b.id != bot.id).cloned().collect()
  } else {
    Vec::new()
  };
  let settings = rt.settings();
  let tz = rt.tz();
  let now = chrono::Utc::now().with_timezone(&tz);
  Ok(Context {
    memory: store::memories::list(pool, &bot.id).await?,
    skills: store::skills::enabled(pool, &bot.id).await?,
    routines: store::routines::for_bot(pool, &bot.id).await?,
    secrets: store::secrets::list(pool, &bot.id).await?.into_iter().map(|s| s.name).collect(),
    history: store::messages::recent(pool, &chat.id, HISTORY).await?,
    plugins: rt.plugins.names().await,
    user: settings.user_name.clone(),
    now: now.format("%A, %B %-d, %Y %-I:%M %p").to_string(),
    zone: tz.name().to_string(),
    workspace: rt.computer.workspace().display().to_string(),
    language: settings.language.clone(),
    model: String::new(),
    bot,
    chat,
    members,
    roster,
  })
}
