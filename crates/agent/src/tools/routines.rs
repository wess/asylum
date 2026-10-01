use super::{arg, def, need, Ctx, Outcome};
use crate::event::Event;
use crate::part::Part;
use crate::queue::{Job, Origin};
use anyhow::{anyhow, Result};
use chat::ToolDef;
use serde_json::{json, Value};
use store::routines::{self, New};

pub fn defs() -> Vec<ToolDef> {
  let trig = json!({"type": "string", "enum": routines::TRIGGERS, "description": "schedule (cron), interval (minutes), webhook, or an app event"});
  let props = json!({
    "name": {"type": "string"},
    "instruction": {"type": "string", "description": "what to do each run, written as a standalone task"},
    "trigger": trig,
    "schedule": {"type": "string", "description": "cron for schedule (\"0 8 * * 1-5\" = weekdays 8:00 AM, user's timezone); minutes for interval; empty otherwise"},
    "filter": {"type": "object", "description": "event triggers: e.g. {\"channel\": \"#eng\", \"contains\": \"deploy\"} or {\"event\": \"pull_request\"}"}
  });
  vec![
    def("create_routine", "Create a routine that runs an instruction on a schedule or when an event arrives. It waits for its first scheduled time.", json!({"type": "object", "properties": props, "required": ["name", "instruction", "trigger"]})),
    def("update_routine", "Change a routine by name.", json!({"type": "object", "properties": props, "required": ["name"]})),
    def("set_routine_active", "Pause or resume a routine.", json!({"type": "object", "properties": {"name": {"type": "string"}, "active": {"type": "boolean"}}, "required": ["name", "active"]})),
    def("delete_routine", "Delete a routine. There is no undo.", json!({"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]})),
    def("list_routines", "List your routines with their schedules and next runs.", json!({"type": "object", "properties": {}})),
    def("test_routine", "Run a routine once now as a test. Test runs do real work.", json!({"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]})),
  ]
}

fn fmt_time(rt: &crate::Runtime, ms: Option<i64>) -> String {
  match ms {
    Some(ms) => {
      let tz = rt.tz();
      chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| t.with_timezone(&tz).format("%a %b %-d, %-I:%M %p").to_string())
        .unwrap_or_default()
    }
    None => "on its next event".into(),
  }
}

pub async fn call(ctx: &Ctx<'_>, name: &str, args: &Value) -> Option<Result<Outcome>> {
  let rt = ctx.rt;
  let pool = &rt.pool;
  let bot = ctx.bot.id.as_str();
  let r = match name {
    "create_routine" => async {
      let trigger = match arg(args, "trigger") {
        "" => routines::SCHEDULE,
        t => t,
      };
      let schedule = arg(args, "schedule");
      let next = schedule::next_run(trigger, schedule, store::now(), rt.tz())?;
      let r = routines::create(pool, New {
        bot: bot.into(),
        name: need(args, "name")?.into(),
        instruction: need(args, "instruction")?.into(),
        trigger: trigger.into(),
        schedule: schedule.into(),
        filter: args.get("filter").cloned().unwrap_or(json!({})),
        next_run: next,
      })
      .await?;
      rt.emit(Event::Routines { bot: bot.into() });
      let when = schedule::describe(&r.trigger, &r.schedule, &r.filter);
      Ok(Outcome::with(
        format!("Created routine \"{}\": {when}. Next run: {}.", r.name, fmt_time(rt, r.next_run)),
        Part::Routine { id: r.id.clone(), name: r.name.clone(), event: "created".into() },
      ))
    }
    .await,
    "update_routine" => async {
      let n = need(args, "name")?;
      let r = routines::find(pool, bot, n).await?.ok_or_else(|| anyhow!("no routine named {n}"))?;
      let trigger = match arg(args, "trigger") {
        "" => r.trigger.clone(),
        t => t.to_string(),
      };
      let schedule = args.get("schedule").and_then(Value::as_str).map(str::to_string).unwrap_or(r.schedule.clone());
      let next = if r.active { schedule::next_run(&trigger, &schedule, store::now(), rt.tz())? } else { None };
      let upd = New {
        bot: bot.into(),
        name: r.name.clone(),
        instruction: match arg(args, "instruction") {
          "" => r.instruction.clone(),
          i => i.to_string(),
        },
        trigger,
        schedule,
        filter: args.get("filter").cloned().unwrap_or(r.filter()),
        next_run: next,
      };
      routines::update(pool, &r.id, &upd).await?;
      rt.emit(Event::Routines { bot: bot.into() });
      Ok(Outcome::with(
        format!("Updated \"{}\". Next run: {}.", r.name, fmt_time(rt, next)),
        Part::Routine { id: r.id, name: r.name, event: "updated".into() },
      ))
    }
    .await,
    "set_routine_active" => async {
      let n = need(args, "name")?;
      let r = routines::find(pool, bot, n).await?.ok_or_else(|| anyhow!("no routine named {n}"))?;
      let on = args["active"].as_bool().unwrap_or(true);
      let next = if on { schedule::next_run(&r.trigger, &r.schedule, store::now(), rt.tz())? } else { None };
      routines::set_active(pool, &r.id, on, next).await?;
      rt.emit(Event::Routines { bot: bot.into() });
      Ok(Outcome::text(format!("{} \"{}\".", if on { "Resumed" } else { "Paused" }, r.name)))
    }
    .await,
    "delete_routine" => async {
      let n = need(args, "name")?;
      let r = routines::find(pool, bot, n).await?.ok_or_else(|| anyhow!("no routine named {n}"))?;
      routines::delete(pool, &r.id).await?;
      rt.emit(Event::Routines { bot: bot.into() });
      Ok(Outcome::text(format!("Deleted \"{}\".", r.name)))
    }
    .await,
    "list_routines" => async {
      let all = routines::for_bot(pool, bot).await?;
      if all.is_empty() {
        return Ok(Outcome::text("No routines yet."));
      }
      let lines: Vec<String> = all
        .iter()
        .map(|r| {
          let when = schedule::describe(&r.trigger, &r.schedule, &r.filter);
          let state = if r.active { format!("next {}", fmt_time(rt, r.next_run)) } else { "Paused".into() };
          format!("- {}: {when} ({state}) — {}", r.name, r.instruction)
        })
        .collect();
      Ok(Outcome::text(lines.join("\n")))
    }
    .await,
    "test_routine" => async {
      let n = need(args, "name")?;
      let r = routines::find(pool, bot, n).await?.ok_or_else(|| anyhow!("no routine named {n}"))?;
      let chat = store::chats::direct(pool, bot).await?;
      crate::turn::enqueue(rt, Job { bot: bot.into(), chat: chat.id, origin: Origin::Test, routine: Some(r.id.clone()), note: String::new() });
      Ok(Outcome::text(format!("Started a test run of \"{}\"; its result will post here.", r.name)))
    }
    .await,
    _ => return None,
  };
  Some(r)
}
