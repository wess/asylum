//! Conversation content export (prompts, responses, and tool I/O) as JSON
//! or Markdown, for one conversation or all of them.

use crate::runtime::Runtime;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use store::{bots, chats, messages};

/// One conversation as JSON.
pub async fn chat_json(rt: &Runtime, chat: &str) -> Result<Value> {
  let c = chats::get(&rt.pool, chat).await?;
  let names: std::collections::HashMap<String, String> = bots::list(&rt.pool).await?.into_iter().map(|b| (b.id, b.name)).collect();
  let msgs = messages::list(&rt.pool, chat).await?;
  let items: Vec<Value> = msgs
    .iter()
    .map(|m| {
      let author = m.bot_id.as_ref().and_then(|b| names.get(b)).cloned().unwrap_or_else(|| m.role.clone());
      let tools: Vec<Value> = m
        .parts()
        .into_iter()
        .filter(|p| p["type"] == "tool")
        .map(|p| json!({ "tool": p["name"], "input": p["args"], "output": p["result"], "status": p["status"] }))
        .collect();
      json!({ "at": m.created, "role": m.role, "author": author, "text": m.body, "status": m.status, "tools": tools })
    })
    .collect();
  Ok(json!({ "id": c.id, "title": c.title, "kind": c.kind, "messages": items }))
}

/// One conversation as Markdown.
pub fn markdown(v: &Value) -> String {
  let mut out = format!("# {}\n\n", v["title"].as_str().unwrap_or("Conversation"));
  for m in v["messages"].as_array().into_iter().flatten() {
    out.push_str(&format!("**{}**\n\n{}\n\n", m["author"].as_str().unwrap_or(""), m["text"].as_str().unwrap_or("")));
    for t in m["tools"].as_array().into_iter().flatten() {
      out.push_str(&format!(
        "<details><summary>{} ({})</summary>\n\nInput:\n\n```json\n{}\n```\n\nOutput:\n\n```\n{}\n```\n</details>\n\n",
        t["tool"].as_str().unwrap_or(""),
        t["status"].as_str().unwrap_or(""),
        serde_json::to_string_pretty(&t["input"]).unwrap_or_default(),
        t["output"].as_str().unwrap_or("")
      ));
    }
  }
  out
}

/// Every conversation into `dir` as `<title>-<id>.json` and `.md`.
pub async fn all(rt: &Runtime, dir: &Path) -> Result<Vec<PathBuf>> {
  std::fs::create_dir_all(dir)?;
  let mut written = Vec::new();
  for c in chats::all(&rt.pool).await? {
    let v = chat_json(rt, &c.id).await?;
    let stem: String = c.title.chars().map(|ch| if ch.is_alphanumeric() { ch.to_ascii_lowercase() } else { '.' }).collect();
    let stem = format!("{}.{}", stem.trim_matches('.'), &c.id[..8.min(c.id.len())]);
    let json_path = dir.join(format!("{stem}.json"));
    std::fs::write(&json_path, serde_json::to_string_pretty(&v)?)?;
    std::fs::write(dir.join(format!("{stem}.md")), markdown(&v))?;
    written.push(json_path);
  }
  crate::audit::change(rt, "user", "conversations.exported", &dir.display().to_string(), &written.len().to_string()).await;
  Ok(written)
}
