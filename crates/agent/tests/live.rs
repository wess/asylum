//! Live checks against a local Ollama (`cargo test -p agent --test live --
//! --ignored --test-threads 1`). Small local models are not deterministic,
//! so these verify the plumbing end to end rather than exact wording.

use agent::{api, Event, Runtime};
use std::time::Duration;

const MODEL: &str = "qwen3.5:9b";

async fn runtime(dir: &std::path::Path) -> Runtime {
  let mut p = provider::preset::build("ollama", "ollama");
  p.models = vec![MODEL.into()];
  let s = config::Settings { providers: vec![p], provider: "ollama".into(), model: MODEL.into(), memory: false, auto_review: false, ..Default::default() };
  Runtime::start(dir.to_path_buf(), s).await.unwrap()
}

async fn settle(rt: &Runtime, bots: &[String]) {
  let mut ev = rt.subscribe();
  let _ = tokio::time::timeout(Duration::from_secs(300), async {
    loop {
      tokio::time::sleep(Duration::from_millis(500)).await;
      if bots.iter().all(|b| !rt.queues.busy(b)) {
        // Give queued follow-ups a moment to start.
        tokio::time::sleep(Duration::from_millis(800)).await;
        if bots.iter().all(|b| !rt.queues.busy(b)) {
          return;
        }
      }
      while let Ok(Event::Approval { id }) = ev.try_recv() {
        let _ = api::cards::approve(rt, &id, true, false).await;
      }
      for a in store::approvals::pending(&rt.pool).await.unwrap_or_default() {
        let _ = api::cards::approve(rt, &a.id, true, false).await;
      }
    }
  })
  .await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn routine_from_chat() {
  let dir = tempfile::tempdir().unwrap();
  let rt = runtime(dir.path()).await;
  let (bot, chat) = api::bots::create(&rt, Some("Digest")).await.unwrap();
  api::chat::send(&rt, &chat.id, "Create a routine named \"Morning digest\" that runs every weekday at 8:00 AM (cron 0 8 * * 1-5) with the instruction: summarize yesterday's notes in the workspace.", &[], None).await.unwrap();
  settle(&rt, std::slice::from_ref(&bot.id)).await;
  let rs = store::routines::for_bot(&rt.pool, &bot.id).await.unwrap();
  assert_eq!(rs.len(), 1, "routine not created");
  assert_eq!(rs[0].trigger, "schedule");
  assert!(rs[0].next_run.is_some());
  println!("routine: {} / {} / {}", rs[0].name, rs[0].schedule, schedule::describe(&rs[0].trigger, &rs[0].schedule, &rs[0].filter));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn handoff_between_bots() {
  let dir = tempfile::tempdir().unwrap();
  let rt = runtime(dir.path()).await;
  let (a, chat) = api::bots::create(&rt, Some("Lead")).await.unwrap();
  let (b, _) = api::bots::create(&rt, Some("Writer")).await.unwrap();
  api::chat::send(&rt, &chat.id, "Use message_bot to ask Writer to write a two-line poem about Rust. Then tell me you handed it off.", &[], None).await.unwrap();
  settle(&rt, &[a.id.clone(), b.id.clone()]).await;
  let writer_chat = store::chats::direct(&rt.pool, &b.id).await.unwrap();
  let msgs = store::messages::list(&rt.pool, &writer_chat.id).await.unwrap();
  assert!(msgs.iter().any(|m| m.bot_id.as_deref() == Some(a.id.as_str())), "handoff message missing in Writer's chat");
  assert!(msgs.iter().any(|m| m.bot_id.as_deref() == Some(b.id.as_str()) && m.role == "bot"), "Writer never replied");
  for m in msgs {
    println!("[{}] {}", m.role, m.body.chars().take(120).collect::<String>());
  }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn group_mentions_route() {
  let dir = tempfile::tempdir().unwrap();
  let rt = runtime(dir.path()).await;
  let (a, _) = api::bots::create(&rt, Some("Alpha")).await.unwrap();
  let (b, _) = api::bots::create(&rt, Some("Beta")).await.unwrap();
  let g = api::bots::create_group(&rt, &[a.id.clone(), b.id.clone()]).await.unwrap();
  api::chat::send(&rt, &g.id, "@Beta say hello in five words.", &[], None).await.unwrap();
  settle(&rt, &[a.id.clone(), b.id.clone()]).await;
  let msgs = store::messages::list(&rt.pool, &g.id).await.unwrap();
  let replies: Vec<_> = msgs.iter().filter(|m| m.role == "bot").collect();
  assert!(replies.iter().all(|m| m.bot_id.as_deref() == Some(b.id.as_str())), "only Beta should answer");
  assert!(!replies.is_empty());
  api::chat::send(&rt, &g.id, "@everyone reply with your name only.", &[], None).await.unwrap();
  settle(&rt, &[a.id.clone(), b.id.clone()]).await;
  let msgs = store::messages::list(&rt.pool, &g.id).await.unwrap();
  assert!(msgs.iter().any(|m| m.bot_id.as_deref() == Some(a.id.as_str())), "Alpha should answer @everyone");
  println!("group title: {}", store::chats::get(&rt.pool, &g.id).await.unwrap().title);
}
