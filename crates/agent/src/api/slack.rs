//! "Bring to your team's Slack": a Team Bot answers its own Slack app. DMs
//! are answered every time (one DM, one conversation); in channels and
//! group DMs it answers when mentioned and then follows that thread. Only
//! linked teammates get answers; others are asked, privately, to link.

use crate::event::Event;
use crate::part::Part;
use crate::runtime::Runtime;
use anyhow::{anyhow, bail, Result};
use slack::route::{self, Decision};
use std::collections::HashSet;
use std::time::Duration;
use store::{chats, messages};

pub fn bot_key(bot: &str) -> String {
  format!("slack-bot-{bot}")
}

pub fn app_key(bot: &str) -> String {
  format!("slack-app-{bot}")
}

/// The manifest link for creating this Team Bot's Slack app.
pub async fn manifest_url(rt: &Runtime, bot: &str) -> Result<String> {
  let b = store::bots::get(&rt.pool, bot).await?;
  Ok(slack::manifest::create_url(&slack::manifest::build(&b.name, &b.description)))
}

/// Connect with the app's bot token (xoxb-) and app-level token (xapp-).
pub async fn connect(rt: &Runtime, bot: &str, bot_token: &str, app_token: &str, base: Option<&str>) -> Result<()> {
  let b = store::bots::get(&rt.pool, bot).await?;
  if !b.is_team() {
    bail!("Only Team Bots can be brought to Slack.");
  }
  if !b.published {
    bail!("Connect after publishing.");
  }
  let base = base.unwrap_or(slack::BASE).to_string();
  let (team, me) = slack::web::whoami(&base, bot_token.trim()).await?;
  slack::socket::open_url(&base, app_token.trim()).await?;
  config::secret::set(&bot_key(bot), bot_token.trim())?;
  config::secret::set(&app_key(bot), app_token.trim())?;
  store::slack::save_app(&rt.pool, bot, &team, &me).await?;
  start(rt, bot, Some(base)).await?;
  rt.emit(Event::BotsChanged);
  Ok(())
}

pub const AWAITING: &str = "awaiting";

/// The workspace requires admin approval: Slack sends the request when the
/// app is installed; this records that setup is waiting on it.
pub async fn request_approval(rt: &Runtime, bot: &str) -> Result<()> {
  store::slack::await_approval(&rt.pool, bot).await?;
  rt.emit(Event::BotsChanged);
  Ok(())
}

/// Where the admin's decision shows: the app's install page in Slack.
pub const APPS_URL: &str = "https://api.slack.com/apps";

pub async fn remove(rt: &Runtime, bot: &str) -> Result<()> {
  if let Some(h) = rt.slack.lock().await.remove(bot) {
    h.abort();
  }
  let _ = config::secret::remove(&bot_key(bot));
  let _ = config::secret::remove(&app_key(bot));
  store::slack::remove_app(&rt.pool, bot).await?;
  rt.emit(Event::BotsChanged);
  Ok(())
}

/// Reconnect every Team Bot that has a Slack app (at launch).
pub async fn start_all(rt: &Runtime) {
  for a in store::slack::apps(&rt.pool).await.unwrap_or_default() {
    if a.status == AWAITING {
      continue;
    }
    let _ = start(rt, &a.bot_id, None).await;
  }
}

pub async fn start(rt: &Runtime, bot: &str, base: Option<String>) -> Result<()> {
  let app = store::slack::app(&rt.pool, bot).await?.ok_or_else(|| anyhow!("not connected to Slack"))?;
  let bot_token = config::secret::get(&bot_key(bot)).ok_or_else(|| anyhow!("Slack token missing; connect again"))?;
  let app_token = config::secret::get(&app_key(bot)).ok_or_else(|| anyhow!("Slack token missing; connect again"))?;
  let base = base.unwrap_or_else(|| slack::BASE.to_string());
  let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
  let socket = tokio::spawn(slack::socket::run(base.clone(), app_token, tx));
  let rt2 = rt.clone();
  let bot_id = bot.to_string();
  let handle = tokio::spawn(async move {
    while let Some(e) = rx.recv().await {
      let (rt, bot, me, token, base) = (rt2.clone(), bot_id.clone(), app.bot_user.clone(), bot_token.clone(), base.clone());
      tokio::spawn(async move {
        if let Err(err) = handle(&rt, &bot, &me, &token, &base, e).await {
          let _ = store::notifications::add(&rt.pool, Some(&bot), None, "error", "Slack", &err.to_string()).await;
        }
      });
    }
    socket.abort();
  });
  if let Some(old) = rt.slack.lock().await.insert(bot.to_string(), handle) {
    old.abort();
  }
  Ok(())
}

async fn handle(rt: &Runtime, bot: &str, me: &str, token: &str, base: &str, e: route::Event) -> Result<()> {
  let followed: HashSet<String> = store::slack::followed(&rt.pool, bot).await?.into_iter().collect();
  let Decision::Answer { conversation, thread, follow } = route::decide(&e, me, &followed) else { return Ok(()) };
  let links = store::slack::links(&rt.pool).await?;
  if !links.is_empty() && !links.iter().any(|l| l.slack_user == e.user) {
    let _ = slack::web::ephemeral(base, token, &e.channel, &e.user, "Link your Slack account in Asylum (Settings → Team Setup → Link) to talk with this Bot.").await;
    if thread.is_some() {
      slack::web::post(base, token, &e.channel, "I can only help teammates who have linked their account.", thread.as_deref()).await?;
    }
    return Ok(());
  }
  let name = slack::web::user_name(base, token, &e.user).await;
  let chat = match store::slack::chat_for(&rt.pool, bot, &conversation).await? {
    Some(c) => c,
    None => {
      let title = if conversation.starts_with("im:") { format!("Slack: {name}") } else { format!("Slack thread {}", &conversation) };
      let c = chats::create_direct(&rt.pool, bot, &title).await?;
      c.id
    }
  };
  store::slack::map_chat(&rt.pool, bot, &conversation, &chat, follow).await?;
  let text = route::clean(&e.text, me);
  let sent = crate::api::chat::send(rt, &chat, &format!("{name} (on Slack): {text}"), &[], None).await?;
  let reply = wait_reply(rt, &chat, sent.created, Duration::from_secs(600)).await?;
  slack::web::post(base, token, &e.channel, &reply, thread.as_deref()).await
}

/// The Bot's finished reply after `since`, as plain text.
async fn wait_reply(rt: &Runtime, chat: &str, since: i64, timeout: Duration) -> Result<String> {
  let start = std::time::Instant::now();
  loop {
    let recent = messages::recent(&rt.pool, chat, 5).await?;
    if let Some(m) = recent.iter().rev().find(|m| m.role == messages::BOT && m.created >= since && m.status != messages::STREAMING) {
      if m.status == messages::ERROR {
        let why = crate::part::parse(&m.parts).into_iter().find_map(|p| match p {
          Part::Error { text } => Some(text),
          _ => None,
        });
        return Ok(why.unwrap_or_else(|| "Sorry, something went wrong.".into()));
      }
      return Ok(if m.body.trim().is_empty() { "Done.".into() } else { m.body.clone() });
    }
    if start.elapsed() > timeout {
      bail!("timed out waiting for the Bot");
    }
    tokio::time::sleep(Duration::from_millis(400)).await;
  }
}
