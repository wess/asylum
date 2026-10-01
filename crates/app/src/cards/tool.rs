//! Tool activity: a compact, expandable row per call.

use crate::chat::ChatPane;
use crate::i18n::{t, tf};
use agent::Part;
use gpui::prelude::*;
use gpui::{AnyElement, Context, SharedString};
use guise::{AIToolCall, AIToolStatus};
use store::Message;

pub fn label(name: &str, args: &serde_json::Value) -> String {
  let pick = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
  let short = |s: String| if s.chars().count() > 70 { format!("{}…", s.chars().take(70).collect::<String>()) } else { s };
  match name {
    "shell" => tf("Ran `{}`", &[&short(pick("command"))]),
    "local_shell" => tf("Ran on your computer: `{}`", &[&short(pick("command"))]),
    "read_file" | "local_read_file" => tf("Read {}", &[&pick("path")]),
    "write_file" => tf("Wrote {}", &[&pick("path")]),
    "edit_file" => tf("Edited {}", &[&pick("path")]),
    "list_files" => tf("Listed {}", &[&if pick("path").is_empty() { t("the workspace").to_string() } else { pick("path") }]),
    "search_files" => tf("Searched files for \"{}\"", &[&pick("query")]),
    "delete_file" => tf("Deleted {}", &[&pick("path")]),
    "move_file" => tf("Moved {} → {}", &[&pick("from"), &pick("to")]),
    "web_search" => tf("Searched the web for \"{}\"", &[&pick("query")]),
    "fetch_url" => tf("Read {}", &[&pick("url")]),
    "browser_open" => tf("Opened {}", &[&pick("url")]),
    "browser_look" => t("Looked at the screen").into(),
    "browser_click" => tf("Clicked [{}]", &[&args["index"].to_string()]),
    "browser_type" => tf("Typed into [{}]", &[&args["index"].to_string()]),
    "browser_press" => tf("Pressed {}", &[&pick("key")]),
    "browser_scroll" => t("Scrolled").into(),
    "browser_back" => t("Went back").into(),
    "browser_screenshot" => t("Took a screenshot").into(),
    "remember" => t("Saved to memory").into(),
    "forget" => t("Updated memory").into(),
    "use_skill" => tf("Used skill /{}", &[&pick("name")]),
    "save_skill" => tf("Saved skill \"{}\"", &[&pick("name")]),
    "message_agent" => tf("Messaged {}", &[&pick("agent")]),
    "create_agent" => tf("Created {}", &[&pick("name")]),
    "create_routine" => tf("Created routine \"{}\"", &[&pick("name")]),
    "generate_image" => t("Generated an image").into(),
    n if n.contains("__") => {
      let (plugin, tool) = n.split_once("__").unwrap_or((n, ""));
      format!("{} · {}", plugin.replace('_', " "), tool.replace('_', " "))
    }
    n => n.replace('_', " "),
  }
}

pub fn render(pane: &mut ChatPane, m: &Message, i: usize, p: &Part, cx: &mut Context<ChatPane>) -> AnyElement {
  let Part::Tool { id, name, args, result, status } = p else { return gpui::div().into_any_element() };
  let key = format!("{}:{i}", m.id);
  let open = pane.open.contains(&key);
  // A message that stopped or failed can't still be running a tool.
  let settled = m.status != "streaming";
  let st = match status.as_str() {
    "running" | "waiting" if settled => AIToolStatus::Error,
    "running" | "waiting" => AIToolStatus::Running,
    "error" | "denied" => AIToolStatus::Error,
    _ => AIToolStatus::Ok,
  };
  let k2 = key.clone();
  AIToolCall::new(SharedString::from(format!("tool-{}-{id}", m.id)), label(name, args))
    .status(st)
    .arguments(serde_json::to_string_pretty(args).unwrap_or_default())
    .result(result.clone())
    .open(open)
    .expandable(true)
    .on_toggle(cx.listener(move |this, _, _, cx| {
      if !this.open.remove(&k2) {
        this.open.insert(k2.clone());
      }
      cx.notify();
    }))
    .into_any_element()
}
