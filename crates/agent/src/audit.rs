//! Recording what happens: Bot actions (metadata only) and control-plane
//! changes. Every action also goes to OpenTelemetry when it's configured.

use crate::runtime::Runtime;
use store::events::{self, New};

/// Repeats of the same change within this window are one entry.
pub const COALESCE_MS: i64 = 30_000;

/// A Bot ran (or was stopped from running) a tool.
pub async fn action(rt: &Runtime, bot: &str, chat: &str, tool: &str, target: &str, outcome: &str, ms: u64) {
  let _ = events::add(&rt.pool, New { kind: events::ACTION, actor: bot, bot: Some(bot), chat: Some(chat), name: tool, target, outcome, detail: &format!("{ms}ms") }).await;
  crate::otel::span(rt, "tool", &[("agent", bot), ("chat", chat), ("tool", tool), ("outcome", outcome)], ms);
}

/// A control-plane change by the user (or the admin's policy).
pub async fn change(rt: &Runtime, actor: &str, name: &str, target: &str, detail: &str) {
  // Typing into a setting saves on each keystroke; log it once.
  if let Ok(last) = events::list(&rt.pool, events::AUDIT, store::now() - COALESCE_MS, 1).await {
    if last.first().is_some_and(|e| e.name == name && e.target == target && e.actor == actor) {
      return;
    }
  }
  let _ = events::add(&rt.pool, New { kind: events::AUDIT, actor, name, target, outcome: "ok", detail, ..Default::default() }).await;
}
