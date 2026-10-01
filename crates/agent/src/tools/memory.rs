use super::{arg, def, need, Ctx, Outcome};
use anyhow::Result;
use chat::ToolDef;
use serde_json::{json, Value};
use store::memories;

pub fn defs() -> Vec<ToolDef> {
  vec![
    def(
      "remember",
      "Save something durable to your memory: a stable preference, an important fact, or a summary of finished work. Not for one-off task details.",
      json!({"type": "object", "properties": {
        "content": {"type": "string"},
        "kind": {"type": "string", "enum": ["preference", "fact", "summary"]},
        "scope": {"type": "string", "enum": ["bot", "team", "person"], "description": "Team Agents only: team memory or notes about this person"}
      }, "required": ["content"]}),
    ),
    def(
      "forget",
      "Remove or correct a memory. Pass the text of the memory (or part of it); give `replacement` to correct it instead.",
      json!({"type": "object", "properties": {"match": {"type": "string"}, "replacement": {"type": "string"}}, "required": ["match"]}),
    ),
  ]
}

pub async fn call(ctx: &Ctx<'_>, name: &str, args: &Value) -> Option<Result<Outcome>> {
  let pool = &ctx.rt.pool;
  let r = match name {
    "remember" => async {
      let content = need(args, "content")?;
      let kind = match arg(args, "kind") {
        "" => "fact",
        k => k,
      };
      let scope = match (arg(args, "scope"), ctx.bot.is_team()) {
        ("team", true) => memories::TEAM,
        ("person", true) => memories::PERSON,
        _ => memories::BOT,
      };
      let person = if scope == memories::PERSON { ctx.rt.settings().user_name } else { String::new() };
      let new = memories::remember_scoped(pool, &ctx.bot.id, kind, scope, &person, content).await?;
      Ok(Outcome::text(if new { "Remembered." } else { "Already remembered." }))
    }
    .await,
    "forget" => async {
      let needle = need(args, "match")?.to_lowercase();
      let all = memories::list(pool, &ctx.bot.id).await?;
      let hits: Vec<_> = all.iter().filter(|m| m.content.to_lowercase().contains(&needle)).collect();
      if hits.is_empty() {
        return Ok(Outcome::text("No memory matched."));
      }
      let replacement = arg(args, "replacement");
      for m in &hits {
        if replacement.is_empty() {
          memories::delete(pool, &m.id).await?;
        } else {
          memories::update(pool, &m.id, replacement).await?;
        }
      }
      Ok(Outcome::text(format!(
        "{} {} memor{}.",
        if replacement.is_empty() { "Removed" } else { "Corrected" },
        hits.len(),
        if hits.len() == 1 { "y" } else { "ies" }
      )))
    }
    .await,
    _ => return None,
  };
  Some(r)
}
