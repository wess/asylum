//! After a turn, the Bot reflects on the exchange and keeps anything durable
//! it did not already save: stable preferences (including how the user likes
//! things written), important facts, and a one-line summary of finished work.

use crate::runtime::Runtime;
use grok::Message;
use serde::Deserialize;
use store::{memories, messages};

#[derive(Deserialize)]
struct Item {
  kind: String,
  content: String,
}

pub fn prompt(known: &[String], exchange: &str) -> String {
  format!(
    "You maintain an AI teammate's long-term memory. From the exchange below, extract only durable items worth \
remembering across future tasks: stable user preferences (including tone, format, and writing voice), important facts \
about the user's work, and — if a task was completed — a one-line summary of it. Skip anything transient, anything \
already known, and all secrets or credentials.\n\nAlready known:\n{}\n\nExchange:\n{exchange}\n\n\
Reply with only a JSON array (possibly empty) of {{\"kind\": \"preference|fact|summary\", \"content\": \"...\"}}.",
    if known.is_empty() { "(nothing)".into() } else { known.iter().map(|k| format!("- {k}")).collect::<Vec<_>>().join("\n") }
  )
}

pub fn parse(text: &str) -> Vec<(String, String)> {
  let (Some(s), Some(e)) = (text.find('['), text.rfind(']')) else { return Vec::new() };
  if e < s {
    return Vec::new();
  }
  serde_json::from_str::<Vec<Item>>(&text[s..=e])
    .unwrap_or_default()
    .into_iter()
    .filter(|i| !i.content.trim().is_empty() && i.content.len() < 500)
    .map(|i| (i.kind, i.content))
    .collect()
}

pub async fn reflect(rt: &Runtime, bot: &str, chat: &str) {
  let Ok(fast) = rt.fast().await else { return };
  let Ok(recent) = messages::recent(&rt.pool, chat, 8).await else { return };
  let exchange: String = recent
    .iter()
    .filter(|m| !m.body.trim().is_empty())
    .map(|m| {
      let who = if m.role == messages::USER { "User" } else { "Bot" };
      let body: String = m.body.chars().take(1500).collect();
      format!("{who}: {body}")
    })
    .collect::<Vec<_>>()
    .join("\n");
  if exchange.len() < 40 {
    return;
  }
  let known: Vec<String> = memories::list(&rt.pool, bot).await.unwrap_or_default().into_iter().map(|m| m.content).collect();
  let Ok(text) = fast.complete(vec![Message::user(prompt(&known, &exchange))], Some(400)).await else { return };
  for (kind, content) in parse(&text) {
    let _ = memories::remember(&rt.pool, bot, &kind, &content).await;
  }
}

#[cfg(test)]
#[path = "../tests/memory.rs"]
mod tests;
