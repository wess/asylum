//! Voice chat around the realtime session: the Bot's instructions going in,
//! and the record (plus any agreed follow-up work) coming out.

use crate::event::Event;
use crate::part::Part;
use crate::queue::{Job, Origin};
use crate::runtime::Runtime;
use anyhow::Result;
use store::messages;

pub const SPOKEN: &str = "\n\n## You're in a live voice chat\nSpeak naturally and briefly: one to three sentences at a time. \
No markdown, lists, or links. If the user asks for work that needs your tools, say you'll do it right after the call.";

pub async fn instructions(rt: &Runtime, bot: &str, chat: &str) -> Result<String> {
  let ctx = crate::context::load(rt, bot, chat).await?;
  Ok(format!("{}{SPOKEN}", crate::prompt::system(&ctx)))
}

/// Record a finished call in the chat and hand any follow-up to the Bot.
pub async fn finish(rt: &Runtime, bot: &str, chat: &str, seconds: u64, transcript: &str) -> Result<()> {
  let part = Part::VoiceChat { seconds, transcript: transcript.to_string() };
  let m = messages::add(&rt.pool, messages::New {
    chat,
    bot: Some(bot),
    role: messages::BOT,
    body: "",
    parts: &crate::part::to_values(&[part]),
    status: messages::DONE,
    run: None,
    thread: None,
  })
  .await?;
  rt.emit(Event::Message { chat: chat.into(), message: m.id });
  if transcript.trim().len() > 20 {
    crate::turn::enqueue(rt, Job {
      bot: bot.into(),
      chat: chat.into(),
      origin: Origin::Chat,
      routine: None,
      note: format!(
        "You just finished a voice chat with the user. Transcript:\n{transcript}\n\nDo any follow-up work you agreed to. If there is none, reply with a one-line recap."
      ),
    });
  }
  Ok(())
}

/// Tools a voice session may call: a single `hand_off` that queues work for
/// after the call.
pub fn tools() -> Vec<serde_json::Value> {
  vec![serde_json::json!({
    "type": "function",
    "name": "note_task",
    "description": "Note a task to do with your tools right after this call.",
    "parameters": {"type": "object", "properties": {"task": {"type": "string"}}, "required": ["task"]}
  })]
}
