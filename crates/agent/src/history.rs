//! Stored messages as model messages. A Bot's own replies become assistant
//! turns (with their tool calls and results replayed); the user's messages
//! and other Bots' messages become user turns, the latter prefixed with the
//! speaker's name. Older tool output is trimmed to keep context affordable.

use crate::part::{self, Part};
use chat::{Message as Msg, Part as MsgPart, ToolCall};
use store::{Bot, Message};

pub const RECENT_TOOLS: usize = 6;
const OLD_RESULT: usize = 1_500;

pub fn build(me: &Bot, history: &[Message], roster: &[Bot], images: &dyn Fn(&str) -> Option<String>) -> Vec<Msg> {
  let name_of = |id: &Option<String>| {
    id.as_ref()
      .and_then(|i| roster.iter().find(|b| &b.id == i))
      .map(|b| b.name.clone())
      .unwrap_or_else(|| "Another Agent".into())
  };
  // Messages whose tool results stay whole: the last few with tools.
  let keep: Vec<usize> = history
    .iter()
    .enumerate()
    .rev()
    .filter(|(_, m)| m.parts.contains("\"tool\""))
    .take(RECENT_TOOLS)
    .map(|(i, _)| i)
    .collect();

  let mut out = Vec::new();
  for (i, m) in history.iter().enumerate() {
    let parts = part::parse(&m.parts);
    match m.role.as_str() {
      "user" => {
        let mut text = m.body.clone();
        let mut imgs = Vec::new();
        for p in &parts {
          if let Part::Attachment { name, path, mime, .. } = p {
            text.push_str(&format!("\n[Attached: {name} at {path}]"));
            if mime.starts_with("image/") {
              if let Some(url) = images(path) {
                imgs.push(MsgPart::image(url));
              }
            }
          }
        }
        if imgs.is_empty() {
          out.push(Msg::user(text));
        } else {
          let mut all = vec![MsgPart::text(text)];
          all.extend(imgs);
          out.push(Msg::user_parts(all));
        }
      }
      "bot" if m.bot_id.as_deref() == Some(me.id.as_str()) => {
        let whole = keep.contains(&i);
        let calls: Vec<(ToolCall, String)> = parts
          .iter()
          .filter_map(|p| match p {
            Part::Tool { id, name, args, result, .. } => {
              let result = if whole { result.clone() } else { trim(result) };
              Some((ToolCall::new(id, name, args.to_string()), result))
            }
            _ => None,
          })
          .collect();
        if !calls.is_empty() {
          out.push(Msg::assistant("", calls.iter().map(|(c, _)| c.clone()).collect()));
          for (c, r) in &calls {
            out.push(Msg::tool(&c.id, if r.is_empty() { "(no output)" } else { r }));
          }
        }
        if !m.body.trim().is_empty() {
          out.push(Msg::assistant(m.body.clone(), Vec::new()));
        }
      }
      "bot" => {
        if !m.body.trim().is_empty() {
          out.push(Msg::user(format!("[{}]: {}", name_of(&m.bot_id), m.body)));
        }
      }
      _ => {
        if !m.body.trim().is_empty() {
          out.push(Msg::user(format!("[event] {}", m.body)));
        }
      }
    }
  }
  merge_users(out)
}

fn trim(s: &str) -> String {
  if s.len() <= OLD_RESULT {
    return s.to_string();
  }
  let mut end = OLD_RESULT;
  while !s.is_char_boundary(end) {
    end -= 1;
  }
  format!("{}… [older output trimmed]", &s[..end])
}

/// Consecutive plain-text user turns merge into one, which models handle
/// better than runs of user messages.
fn merge_users(msgs: Vec<Msg>) -> Vec<Msg> {
  let mut out: Vec<Msg> = Vec::new();
  for m in msgs {
    if let (Some(last), chat::Role::User) = (out.last_mut(), m.role) {
      if last.role == chat::Role::User {
        if let (Some(chat::Content::Text(a)), Some(chat::Content::Text(b))) = (&mut last.content, &m.content) {
          a.push_str("\n\n");
          a.push_str(b);
          continue;
        }
      }
    }
    out.push(m);
  }
  out
}

#[cfg(test)]
#[path = "../tests/history.rs"]
mod tests;
