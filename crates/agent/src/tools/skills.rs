use super::{arg, def, need, Ctx, Outcome};
use crate::event::Event;
use anyhow::{anyhow, Result};
use grok::ToolDef;
use serde_json::{json, Value};
use store::skills;

pub fn defs() -> Vec<ToolDef> {
  vec![
    def("use_skill", "Load a skill's full instructions by name.", json!({"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]})),
    def(
      "save_skill",
      "Save a reusable process as a skill in the shared library and enable it for yourself. Write instructions covering: when to use it, required inputs and access, the steps, validation, output, and what needs approval.",
      json!({"type": "object", "properties": {
        "name": {"type": "string"},
        "description": {"type": "string", "description": "one line: what it does"},
        "instructions": {"type": "string"}
      }, "required": ["name", "description", "instructions"]}),
    ),
    def("list_skills", "List every skill in the library and whether it is enabled for you.", json!({"type": "object", "properties": {}})),
  ]
}

pub async fn call(ctx: &Ctx<'_>, name: &str, args: &Value) -> Option<Result<Outcome>> {
  let pool = &ctx.rt.pool;
  let r = match name {
    "use_skill" => async {
      let n = need(args, "name")?;
      let s = skills::find(pool, n).await?.ok_or_else(|| anyhow!("no skill named {n}"))?;
      Ok(Outcome::text(format!("# {}\n{}\n\n{}", s.name, s.description, s.instructions)))
    }
    .await,
    "save_skill" => async {
      let n = need(args, "name")?;
      let source = skills::LEARNED;
      let s = skills::save(pool, n, arg(args, "description"), need(args, "instructions")?, source).await?;
      skills::enable(pool, &ctx.bot.id, &s.id, true).await?;
      if ctx.bot.is_team() {
        skills::set_team(pool, &s.id, true).await?;
      }
      ctx.rt.emit(Event::Skills);
      Ok(Outcome::text(format!("Saved skill \"{}\". Anyone can call it with /{}.", s.name, s.name)))
    }
    .await,
    "list_skills" => async {
      let all = skills::all(pool).await?;
      let mine: Vec<String> = skills::enabled(pool, &ctx.bot.id).await?.into_iter().map(|s| s.id).collect();
      if all.is_empty() {
        return Ok(Outcome::text("The skill library is empty."));
      }
      let lines: Vec<String> = all
        .iter()
        .map(|s| format!("- {}{}: {}", s.name, if mine.contains(&s.id) { " (enabled)" } else { "" }, s.description))
        .collect();
      Ok(Outcome::text(lines.join("\n")))
    }
    .await,
    _ => return None,
  };
  Some(r)
}
