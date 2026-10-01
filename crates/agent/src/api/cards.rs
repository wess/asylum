//! Answers to the cards a Bot shows: approvals, takeover, secrets, forms,
//! drafts, and skill drafts.

use crate::event::Event;
use crate::part::{self, Part};
use crate::runtime::{Reply, Runtime};
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use store::messages;

pub async fn approve(rt: &Runtime, approval: &str, allow: bool, always: bool) -> Result<()> {
  let reply = if allow { Reply::Approve { always } } else { Reply::Deny };
  if !rt.reply(approval, reply).await {
    // Nothing waiting (the app restarted): record the decision anyway.
    store::approvals::decide(&rt.pool, approval, if allow { store::approvals::APPROVED } else { store::approvals::DENIED }).await?;
  }
  rt.emit(Event::Approval { id: approval.into() });
  Ok(())
}

pub async fn takeover(rt: &Runtime, bot: &str, done: bool) -> Result<()> {
  rt.reply(&format!("takeover:{bot}"), Reply::Takeover(done)).await;
  Ok(())
}

/// Save a secret from a secret card (or the Secrets panel). Optionally fill
/// it into the Bot's current page at an element number.
pub async fn save_secret(rt: &Runtime, bot: &str, name: &str, description: &str, value: &str, fill: Option<u32>) -> Result<()> {
  let team = store::bots::get(&rt.pool, bot).await?.is_team();
  if team {
    store::secrets::check_team(&rt.pool, bot, name, value).await?;
    store::secrets::put_limited(&rt.pool, bot, name, description, store::secrets::TEAM_MAX).await?;
  } else {
    store::secrets::check_value(value)?;
    store::secrets::put(&rt.pool, bot, name, description).await?;
  }
  config::secret::set(&store::secrets::key(bot, name), value).map_err(|_| anyhow!("The secret was not saved. Try again."))?;
  store::secrets::set_size(&rt.pool, bot, name, value.len()).await?;
  let filled = match fill {
    Some(i) => Some(rt.browser.fill_secret(bot, i, value).await.is_ok()),
    None => None,
  };
  rt.reply(&format!("secret:{bot}:{name}"), Reply::Secret { filled }).await;
  rt.emit(Event::BotsChanged);
  Ok(())
}

pub async fn remove_secret(rt: &Runtime, bot: &str, name: &str) -> Result<()> {
  store::secrets::remove(&rt.pool, bot, name).await?;
  config::secret::remove(&store::secrets::key(bot, name))?;
  rt.emit(Event::BotsChanged);
  Ok(())
}

pub async fn skip_secret(rt: &Runtime, bot: &str, name: &str) -> Result<()> {
  rt.reply(&format!("secret:{bot}:{name}"), Reply::Cancel).await;
  Ok(())
}

async fn edit_part(rt: &Runtime, message: &str, index: usize, f: impl FnOnce(&mut Part) -> Result<()>) -> Result<messages::Message> {
  let m = messages::get(&rt.pool, message).await?;
  let mut parts = part::parse(&m.parts);
  let p = parts.get_mut(index).ok_or_else(|| anyhow!("that card is gone"))?;
  f(p)?;
  messages::update(&rt.pool, message, &m.body, &part::to_values(&parts), &m.status).await?;
  rt.emit(Event::Message { chat: m.chat_id.clone(), message: message.into() });
  Ok(m)
}

/// Submit a form card: record the values and send them to the Bot.
pub async fn submit_form(rt: &Runtime, message: &str, index: usize, values: Value) -> Result<()> {
  let mut title = String::new();
  let m = edit_part(rt, message, index, |p| match p {
    Part::Form { title: t, status, values: v, .. } => {
      title = t.clone();
      *status = "submitted".into();
      *v = values.clone();
      Ok(())
    }
    _ => bail!("not a form"),
  })
  .await?;
  let lines: Vec<String> = values
    .as_object()
    .map(|o| o.iter().map(|(k, v)| format!("- {k}: {}", v.as_str().map(str::to_string).unwrap_or(v.to_string()))).collect())
    .unwrap_or_default();
  crate::api::chat::send(rt, &m.chat_id, &format!("{title}:\n{}", lines.join("\n")), &[], None).await?;
  Ok(())
}

/// Send a draft (email or Slack) after the user's edits, through the
/// connected app; email falls back to the system mail client.
pub async fn send_draft(rt: &Runtime, message: &str, index: usize, to: Vec<String>, subject: &str, body: &str, channel: &str) -> Result<String> {
  let m = messages::get(&rt.pool, message).await?;
  let kind = match part::parse(&m.parts).get(index) {
    Some(Part::Draft { kind, .. }) => kind.clone(),
    _ => bail!("not a draft"),
  };
  let offered = rt.plugins.offered(rt).await;
  let find = |needles: &[&str]| offered.iter().find(|o| needles.iter().all(|n| o.name.contains(n))).cloned();
  let how = if kind == "email" {
    match find(&["send", "mail"]).or_else(|| find(&["gmail", "send"])) {
      Some(o) => {
        let args = json!({ "to": to, "subject": subject, "body": body });
        let (text, err) = rt.plugins.call(rt, &o, args).await?;
        if err {
          bail!(text);
        }
        "sent".to_string()
      }
      None => {
        let url = format!(
          "mailto:{}?subject={}&body={}",
          to.join(","),
          urlencode(subject),
          urlencode(body)
        );
        let _ = std::process::Command::new("open").arg(url).status();
        "opened".to_string()
      }
    }
  } else {
    let o = find(&["slack", "post"]).or_else(|| find(&["slack", "message"])).ok_or_else(|| anyhow!("Connect Slack in Marketplace to send this."))?;
    let args = json!({ "channel_id": channel.trim_start_matches('#'), "channel": channel, "text": body });
    let (text, err) = rt.plugins.call(rt, &o, args).await?;
    if err {
      bail!(text);
    }
    "sent".to_string()
  };
  let (to2, subject2, body2, channel2) = (to.clone(), subject.to_string(), body.to_string(), channel.to_string());
  edit_part(rt, message, index, move |p| {
    if let Part::Draft { to, subject, body, channel, status, .. } = p {
      *to = to2;
      *subject = subject2;
      *body = body2;
      *channel = channel2;
      *status = how.clone();
    }
    Ok(())
  })
  .await?;
  Ok(kind)
}

pub async fn discard_draft(rt: &Runtime, message: &str, index: usize) -> Result<()> {
  edit_part(rt, message, index, |p| {
    if let Part::Draft { status, .. } = p {
      *status = "discarded".into();
    }
    Ok(())
  })
  .await?;
  Ok(())
}

/// Save (or discard) a skill drafted from a Teach-a-task demo.
pub async fn save_skill_draft(rt: &Runtime, message: &str, index: usize, name: &str, description: &str, instructions: &str, keep: bool) -> Result<()> {
  let m = edit_part(rt, message, index, |p| {
    if let Part::SkillDraft { name: n, description: d, instructions: i, status } = p {
      *n = name.into();
      *d = description.into();
      *i = instructions.into();
      *status = if keep { "saved".into() } else { "discarded".into() };
    }
    Ok(())
  })
  .await?;
  if keep {
    let s = store::skills::save(&rt.pool, name, description, instructions, store::skills::TAUGHT).await?;
    if let Some(bot) = &m.bot_id {
      store::skills::enable(&rt.pool, bot, &s.id, true).await?;
    }
    rt.emit(Event::Skills);
  }
  Ok(())
}

fn urlencode(s: &str) -> String {
  s.bytes()
    .map(|b| match b {
      b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
      _ => format!("%{b:02X}"),
    })
    .collect()
}
