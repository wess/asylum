//! `asylumdev demo`: fill a separate data folder (`ASYLUM_DATA`) with
//! example Agents, conversations, and cards, for screenshots. Refuses to
//! touch the real data folder.

use agent::Part;
use serde_json::json;
use store::{approvals, bots, chats, memories, messages, routines, sections};

const MIN: i64 = 60_000;

pub async fn seed(rt: &agent::Runtime) -> anyhow::Result<()> {
  if std::env::var_os("ASYLUM_DATA").is_none() {
    anyhow::bail!("set ASYLUM_DATA to a separate folder first; the demo never writes to your real data");
  }
  if bots::count(&rt.pool).await? > 0 {
    anyhow::bail!("this data folder already has Agents");
  }
  let pool = &rt.pool;
  let now = store::now();
  let product = sections::create(pool, "Product").await?;
  let ops = sections::create(pool, "Operations").await?;
  let agent = |name: &str, label: &str, avatar: &str, color: &str, description: &str| bots::Profile {
    name: name.into(),
    label: label.into(),
    avatar: avatar.into(),
    color: color.into(),
    description: description.into(),
  };
  let chief = bots::create(pool, &agent("Chief of Staff", "Runs your day and routes work", "sprite:sage;primary=#6b3fa0;accent=#3b3f8f;hair=#d8d8d8;skin=#f2c7a0", "#7c5cff", "Send a morning digest, flag decisions I owe, and hand work to the right Agent.")).await?;
  let research = bots::create(pool, &agent("Researcher", "Answers questions with sources", "sprite:wizard;primary=#2f5fb3;accent=#3b3f8f;hair=#d8d8d8;skin=#f2c7a0", "#1fb6ff", "Cite every claim. Prefer primary sources.")).await?;
  let bug = bots::create(pool, &agent("Bug Reproducer", "Turns reports into repro steps", "sprite:warrior;primary=#b83b3b;accent=#c9cfd8;hair=#e6a23c;skin=#f2c7a0", "#ff7849", "Reproduce on the computer, bisect when you can, and never comment publicly without approval.")).await?;
  let writer = bots::create(pool, &agent("Writer", "Release notes and docs", "sprite:thief;primary=#2a9d8f;accent=#c2185b;hair=#4a3426;skin=#e8b48c", "#13ce66", "Plain language, short sentences.")).await?;
  let help = bots::create(pool, &agent("Helpdesk", "Answers IT questions for the team", "sprite:monk;primary=#e07b2c;accent=#b83b3b;hair=#2b2b2b;skin=#a8704a", "#ffc82c", "Be brief and link the runbook.")).await?;
  bots::pin(pool, &chief.id, true).await?;
  for b in [&research, &bug, &writer] {
    bots::set_section(pool, &b.id, Some(&product.id)).await?;
  }
  bots::set_section(pool, &help.id, Some(&ops.id)).await?;
  bots::set_kind(pool, &help.id, bots::TEAM).await?;
  bots::set_published(pool, &help.id, true).await?;
  bots::set_model(pool, &research.id, "anthropic", "claude-opus-4").await?;
  bots::set_model(pool, &writer.id, "litellm", "gpt-4o").await?;

  let say = |chat: &str, who: Option<&str>, body: &str, parts: Vec<Part>, at: i64| {
    let parts = agent::part::to_values(&parts);
    let (chat, who, body) = (chat.to_string(), who.map(str::to_string), body.to_string());
    async move {
      let role = if who.is_some() { messages::BOT } else { messages::USER };
      let m = messages::add(pool, messages::New { chat: &chat, bot: who.as_deref(), role, body: &body, parts: &parts, status: messages::DONE, run: None, thread: None }).await?;
      sqlx_set_created(pool, &m.id, at).await
    }
  };

  // The Bug Reproducer, mid-task, waiting on an approval.
  let c = chats::direct(pool, &bug.id).await?;
  say(&c.id, None, "Reproduce #482 (checkout fails on Safari) and tell me which commit broke it.", vec![], now - 14 * MIN).await?;
  let tool = |name: &str, args: serde_json::Value, result: &str| Part::Tool { id: store::newid(), name: name.into(), args, result: result.into(), status: "done".into() };
  let ask = approvals::request(pool, approvals::New {
    bot: bug.id.clone(),
    chat: Some(c.id.clone()),
    run: None,
    kind: approvals::APPROVAL.into(),
    tool: "github__add_issue_comment".into(),
    target: "acme/shop#482".into(),
    args: json!({"issue": 482, "body": "Bisected to 9f3c2e1: the Safari date parser change. Repro steps and a fix are in the thread."}).to_string(),
    reason: String::new(),
    interactive: true,
  })
  .await?;
  say(
    &c.id,
    Some(&bug.id),
    "Reproduced it in Safari's engine on the computer and bisected from v2.3.0. The break is **9f3c2e1** — the date parser now rejects `YYYY-MM-DD` without a time, which Safari sends from the checkout form.\n\n1. Add an item to the cart\n2. Pick a delivery date\n3. Press **Pay** → `Invalid date` (Chrome accepts it, Safari doesn't)\n\nI'd like to post this on the issue.",
    vec![
      tool("shell", json!({"command": "git clone git@github.com:acme/shop && cd shop && npm ci"}), "added 1,204 packages in 21s"),
      tool("browser_open", json!({"url": "http://localhost:3000/checkout"}), "Checkout · 14 elements"),
      tool("shell", json!({"command": "git bisect start v2.4.1 v2.3.0 && git bisect run ./repro.sh"}), "9f3c2e1 is the first bad commit"),
      Part::Approval { id: ask.id.clone() },
    ],
    now - 6 * MIN,
  )
  .await?;
  chats::set_attention(pool, &c.id, true).await?;

  // The Researcher, with sources.
  let r = chats::direct(pool, &research.id).await?;
  say(&r.id, None, "Compare the three most popular Rust web frameworks on performance, ergonomics, and ecosystem.", vec![], now - 90 * MIN).await?;
  say(
    &r.id,
    Some(&research.id),
    "**Axum** is the default choice for new services: tower middleware, great ergonomics, and the largest ecosystem growth this year. **Actix Web** still leads raw throughput benchmarks. **Rocket** is the most beginner-friendly but slower to adopt async features.\n\nRecommendation: Axum, unless you need the last 10% of throughput.",
    vec![
      tool("web_search", json!({"query": "rust web framework benchmarks 2026"}), "8 results"),
      Part::Link { url: "https://github.com/tokio-rs/axum".into(), title: "tokio-rs/axum: Ergonomic and modular web framework".into() },
      Part::Link { url: "https://actix.rs".into(), title: "Actix Web — a powerful, pragmatic web framework".into() },
    ],
    now - 86 * MIN,
  )
  .await?;
  chats::set_unread(pool, &r.id, true).await?;

  // A group chat with a handoff.
  let g = chats::create_group(pool, "Launch notes", &[research.id.clone(), writer.id.clone()]).await?;
  say(&g.id, None, "@Researcher pull together what shipped in 2.4, then hand it to @Writer for release notes.", vec![], now - 40 * MIN).await?;
  say(&g.id, Some(&research.id), "Found 11 merged changes since v2.3.0. Handing the list to Writer.", vec![Part::Handoff { from: research.name.clone(), to: writer.name.clone(), direction: "out".into() }], now - 37 * MIN).await?;
  say(&g.id, Some(&writer.id), "Draft is in `release-notes-2.4.md`: three highlights, five fixes, and the Safari checkout fix called out first.", vec![], now - 33 * MIN).await?;

  // The Chief of Staff's morning routine and memory.
  let cs = chats::direct(pool, &chief.id).await?;
  let next = schedule::next_run("schedule", "0 8 * * 1-5", now, rt.tz()).ok().flatten();
  let routine = routines::create(pool, routines::New { bot: chief.id.clone(), name: "Morning digest".into(), instruction: "Summarize what changed overnight and what I owe today.".into(), trigger: "schedule".into(), schedule: "0 8 * * 1-5".into(), filter: json!({}), next_run: next }).await?;
  say(&cs.id, None, "Every weekday at 8, send me what changed overnight and what I owe today.", vec![], now - 3 * 60 * MIN).await?;
  say(&cs.id, Some(&chief.id), "Done — I'll send it every weekday at 8:00 AM.", vec![Part::Routine { id: routine.id.clone(), name: routine.name.clone(), event: "created".into() }], now - 3 * 60 * MIN + MIN).await?;
  memories::add(pool, &chief.id, "preference", "Keeps the digest under 200 words, decisions first.").await?;
  memories::add(pool, &bug.id, "fact", "acme/shop builds with npm ci; repro script lives in ./repro.sh.").await?;
  chats::direct(pool, &writer.id).await?;
  chats::direct(pool, &help.id).await?;
  Ok(())
}

async fn sqlx_set_created(pool: &store::Pool, id: &str, at: i64) -> anyhow::Result<()> {
  store::messages::set_created(pool, id, at).await
}
