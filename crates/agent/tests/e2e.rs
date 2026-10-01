//! The whole turn engine against a mock xAI API: a user message leads to a
//! streamed tool call (write_file), Auto-review approves it, the file lands
//! in the workspace, and the final reply is stored with its tool card.

use agent::api;
use agent::Event;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn sse(chunks: &[Value]) -> String {
  let mut s: String = chunks.iter().map(|c| format!("data: {c}\n\n")).collect();
  s.push_str("data: [DONE]\n\n");
  s
}

async fn mock() -> String {
  let streamed = Arc::new(AtomicUsize::new(0));
  let app = Router::new()
    .route("/v1/models", get(|| async { Json(json!({"data": [{"id": "grok-4"}, {"id": "grok-3-mini"}]})) }))
    .route(
      "/v1/chat/completions",
      post(move |Json(body): Json<Value>| {
        let streamed = streamed.clone();
        async move {
          if body["stream"] != json!(true) {
            let content = json!({"decision": "proceed", "reason": "requested"}).to_string();
            let reply = json!({"choices": [{"message": {"role": "assistant", "content": content}}]});
            return ([("content-type", "application/json")], reply.to_string());
          }
          let n = streamed.fetch_add(1, Ordering::SeqCst);
          let text = if n == 0 {
            let args = json!({"path": "notes/plan.md", "content": "# Plan\n"}).to_string();
            sse(&[
              json!({"choices": [{"delta": {"content": "On it."}}]}),
              json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "id": "c1", "function": {"name": "write_file", "arguments": args}}]}, "finish_reason": "tool_calls"}]}),
              json!({"choices": [], "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}}),
            ])
          } else {
            sse(&[json!({"choices": [{"delta": {"content": "Wrote the plan."}, "finish_reason": "stop"}]})])
          };
          ([("content-type", "text/event-stream")], text)
        }
      }),
    );
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = listener.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  format!("http://{addr}/v1")
}

#[tokio::test(flavor = "multi_thread")]
async fn message_to_tool_to_reply() {
  std::env::set_var("LITELLM_API_KEY", "test-key");
  let base = mock().await;
  let dir = tempfile::tempdir().unwrap();
  // A LiteLLM-style profile pointed at the mock.
  let mut profile = provider::preset::build("lite-llm", "litellm");
  profile.endpoint = base;
  let settings = config::Settings {
    providers: vec![profile],
    provider: "litellm".into(),
    model: "grok-4".into(),
    memory: false,
    ..Default::default()
  };
  let rt = agent::Runtime::start(dir.path().to_path_buf(), settings).await.unwrap();
  let mut events = rt.subscribe();

  let (bot, chat) = api::bots::create(&rt, Some("Planner")).await.unwrap();
  api::chat::send(&rt, &chat.id, "write a plan", &[], None).await.unwrap();

  let done = tokio::time::timeout(Duration::from_secs(20), async {
    loop {
      if let Ok(Event::BotStatus { bot: b, status }) = events.recv().await {
        if b == bot.id && status == "done" {
          return;
        }
      }
    }
  })
  .await;
  assert!(done.is_ok(), "the Agent never finished");

  let written = rt.computer.workspace().join("notes/plan.md");
  assert_eq!(std::fs::read_to_string(written).unwrap(), "# Plan\n");

  let msgs = store::messages::list(&rt.pool, &chat.id).await.unwrap();
  let reply = msgs.iter().find(|m| m.role == "bot").unwrap();
  assert_eq!(reply.status, "done");
  assert!(reply.body.contains("On it."));
  assert!(reply.body.contains("Wrote the plan."));
  let parts = agent::part::parse(&reply.parts);
  assert!(parts.iter().any(|p| matches!(p, agent::Part::Tool { name, status, .. } if name == "write_file" && status == "done")));
  assert!(parts.iter().any(|p| matches!(p, agent::Part::File { name, .. } if name == "plan.md")));

  let chat_after = store::chats::get(&rt.pool, &chat.id).await.unwrap();
  assert!(chat_after.unread);
  let usage = agent::usage::summary(&rt).await.unwrap();
  assert_eq!(usage.week, 15);
}

/// A Bot pinned to a CLI agent (Claude Code's stream-json shape): the
/// transcript goes in on stdin, streamed text comes back, no tools offered.
#[tokio::test(flavor = "multi_thread")]
async fn process_provider_turn() {
  let dir = tempfile::tempdir().unwrap();
  let script = r#"cat >/dev/null; printf '%s\n' '{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Handled by the CLI."}}}' '{"type":"result","result":"Handled by the CLI.","usage":{"input_tokens":7,"output_tokens":4}}'"#;
  let mut cli = provider::preset::build("claude-code", "claude");
  cli.command = "sh".into();
  cli.args = vec!["-c".into(), script.into()];
  let settings = config::Settings { providers: vec![cli], provider: "claude".into(), model: "sonnet".into(), memory: false, ..Default::default() };
  let rt = agent::Runtime::start(dir.path().to_path_buf(), settings).await.unwrap();
  let mut events = rt.subscribe();
  let (bot, chat) = api::bots::create(&rt, Some("Coder")).await.unwrap();
  api::chat::send(&rt, &chat.id, "fix the build", &[], None).await.unwrap();
  let done = tokio::time::timeout(Duration::from_secs(20), async {
    loop {
      if let Ok(Event::BotStatus { bot: b, status }) = events.recv().await {
        if b == bot.id && status == "done" {
          return;
        }
      }
    }
  })
  .await;
  assert!(done.is_ok(), "the Agent never finished");
  let msgs = store::messages::list(&rt.pool, &chat.id).await.unwrap();
  let reply = msgs.iter().find(|m| m.role == "bot").unwrap();
  assert_eq!(reply.body, "Handled by the CLI.");
  assert_eq!(agent::usage::summary(&rt).await.unwrap().week, 11);
}

/// A webhook POST with the routine's key starts a run carrying the JSON
/// body; a wrong key is refused.
#[tokio::test(flavor = "multi_thread")]
async fn webhook_starts_routine() {
  std::env::set_var("LITELLM_API_KEY", "test-key");
  let base = mock().await;
  let dir = tempfile::tempdir().unwrap();
  let mut profile = provider::preset::build("lite-llm", "litellm");
  profile.endpoint = base;
  let port = {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
  };
  let settings = config::Settings { providers: vec![profile], provider: "litellm".into(), model: "grok-4".into(), memory: false, webhook_port: port, ..Default::default() };
  let rt = agent::Runtime::start(dir.path().to_path_buf(), settings).await.unwrap();
  let srv = rt.clone();
  tokio::spawn(async move { agent::webhook::serve(srv).await });
  tokio::time::sleep(Duration::from_millis(200)).await;
  let (bot, chat) = api::bots::create(&rt, Some("Pager")).await.unwrap();
  let r = store::routines::create(&rt.pool, store::routines::New {
    bot: bot.id.clone(),
    name: "On deploy".into(),
    instruction: "Summarize the deploy event.".into(),
    trigger: "webhook".into(),
    ..Default::default()
  })
  .await
  .unwrap();
  let url = agent::webhook::url(port, &r.id);
  let http = reqwest::Client::new();
  let bad = http.post(&url).bearer_auth("wrong").json(&serde_json::json!({})).send().await.unwrap();
  assert_eq!(bad.status(), 401);
  let mut events = rt.subscribe();
  let ok = http.post(&url).bearer_auth(&r.webhook_key).json(&serde_json::json!({"service": "api", "version": "1.2.3"})).send().await.unwrap();
  assert_eq!(ok.status(), 200);
  assert_eq!(ok.json::<serde_json::Value>().await.unwrap()["started"], true);
  let done = tokio::time::timeout(Duration::from_secs(20), async {
    loop {
      if let Ok(Event::BotStatus { bot: b, status }) = events.recv().await {
        if b == bot.id && status == "done" {
          return;
        }
      }
    }
  })
  .await;
  assert!(done.is_ok());
  let runs = store::runs::for_routine(&rt.pool, &r.id).await.unwrap();
  assert_eq!(runs.len(), 1);
  assert_eq!(runs[0].origin, "routine");
  let msgs = store::messages::list(&rt.pool, &chat.id).await.unwrap();
  let reply = msgs.iter().find(|m| m.role == "bot").unwrap();
  assert!(agent::part::parse(&reply.parts).iter().any(|p| matches!(p, agent::Part::Routine { event, .. } if event == "run")));
}

/// A due scheduled routine fires from the ticker and gets a new next run.
#[tokio::test(flavor = "multi_thread")]
async fn schedule_fires_routine() {
  std::env::set_var("LITELLM_API_KEY", "test-key");
  let base = mock().await;
  let dir = tempfile::tempdir().unwrap();
  let mut profile = provider::preset::build("lite-llm", "litellm");
  profile.endpoint = base;
  let settings = config::Settings { providers: vec![profile], provider: "litellm".into(), model: "grok-4".into(), memory: false, ..Default::default() };
  let rt = agent::Runtime::start(dir.path().to_path_buf(), settings).await.unwrap();
  let (bot, _) = api::bots::create(&rt, Some("Clock")).await.unwrap();
  let r = store::routines::create(&rt.pool, store::routines::New {
    bot: bot.id.clone(),
    name: "Hourly".into(),
    instruction: "Check in.".into(),
    trigger: "schedule".into(),
    schedule: "0 * * * *".into(),
    next_run: Some(store::now() - 1000),
    ..Default::default()
  })
  .await
  .unwrap();
  agent::ticker::spawn(rt.clone());
  let fired = tokio::time::timeout(Duration::from_secs(20), async {
    loop {
      tokio::time::sleep(Duration::from_millis(200)).await;
      if !store::runs::for_routine(&rt.pool, &r.id).await.unwrap().is_empty() && !rt.queues.busy(&bot.id) {
        return;
      }
    }
  })
  .await;
  assert!(fired.is_ok(), "routine never ran");
  let after = store::routines::get(&rt.pool, &r.id).await.unwrap();
  assert!(after.next_run.unwrap() > store::now());
  assert!(after.last_run.is_some());
}

/// Two Bots in one group, each on its own provider: one on a LiteLLM-style
/// endpoint as "gpt-5", one on a Claude-Code-style CLI as "opus".
#[tokio::test(flavor = "multi_thread")]
async fn bots_use_their_own_providers() {
  use std::sync::Mutex;
  std::env::set_var("LITELLM_API_KEY", "test-key");
  let seen: Arc<Mutex<Vec<String>>> = Arc::default();
  let seen2 = seen.clone();
  let app = Router::new().route(
    "/v1/chat/completions",
    post(move |Json(body): Json<Value>| {
      let seen = seen2.clone();
      async move {
        seen.lock().unwrap().push(body["model"].as_str().unwrap_or_default().to_string());
        let text = sse(&[json!({"choices": [{"delta": {"content": "GPT here."}, "finish_reason": "stop"}]})]);
        ([("content-type", "text/event-stream")], text)
      }
    }),
  );
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = listener.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let mut gpt = provider::preset::build("lite-llm", "litellm");
  gpt.endpoint = format!("http://{addr}/v1");
  let mut claude = provider::preset::build("claude-code", "claude");
  claude.command = "sh".into();
  // Echo back the model the CLI was launched with.
  let script = r#"cat >/dev/null; printf '{"type":"result","result":"Opus here as %s.","usage":{"input_tokens":1,"output_tokens":1}}\n' "$1""#;
  claude.args = vec!["-c".into(), script.into(), "sh".into(), "{model}".into()];
  let dir = tempfile::tempdir().unwrap();
  let settings = config::Settings { providers: vec![gpt, claude], provider: "litellm".into(), model: "gpt-4o".into(), memory: false, ..Default::default() };
  let rt = agent::Runtime::start(dir.path().to_path_buf(), settings).await.unwrap();
  let (a, _) = api::bots::create(&rt, Some("Gee")).await.unwrap();
  let (b, _) = api::bots::create(&rt, Some("Ope")).await.unwrap();
  store::bots::set_model(&rt.pool, &a.id, "litellm", "gpt-5").await.unwrap();
  store::bots::set_model(&rt.pool, &b.id, "claude", "opus").await.unwrap();
  let g = api::bots::create_group(&rt, &[a.id.clone(), b.id.clone()]).await.unwrap();
  api::chat::send(&rt, &g.id, "@everyone say hi", &[], None).await.unwrap();
  let done = tokio::time::timeout(Duration::from_secs(20), async {
    loop {
      tokio::time::sleep(Duration::from_millis(200)).await;
      let msgs = store::messages::list(&rt.pool, &g.id).await.unwrap();
      if msgs.iter().filter(|m| m.role == "bot" && m.status == "done").count() == 2 {
        return msgs;
      }
    }
  })
  .await
  .expect("both Agents should answer");
  let by = |id: &str| done.iter().find(|m| m.bot_id.as_deref() == Some(id)).unwrap().body.clone();
  assert_eq!(by(&a.id), "GPT here.");
  assert_eq!(by(&b.id), "Opus here as opus.");
  // One turn on gpt-5 (Gee's pin); any other request is the default model
  // doing background work such as naming the group.
  let seen = seen.lock().unwrap();
  assert_eq!(seen.iter().filter(|m| *m == "gpt-5").count(), 1, "{seen:?}");
  assert!(seen.iter().all(|m| m == "gpt-5" || m == "gpt-4o"), "{seen:?}");
}

/// A published Team Bot answers a Slack DM: Socket Mode delivers the DM,
/// the Bot replies in Asylum, and the reply is posted back to Slack.
#[tokio::test(flavor = "multi_thread")]
async fn slack_dm_round_trip() {
  use axum::extract::ws::{Message as AxMsg, WebSocketUpgrade};
  use std::sync::Mutex;
  std::env::set_var("LITELLM_API_KEY", "test-key");
  let llm = mock().await;
  let posted: Arc<Mutex<Vec<Value>>> = Arc::default();
  let posted2 = posted.clone();
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = listener.local_addr().unwrap();
  let slack = Router::new()
    .route("/api/auth.test", post(|| async { Json(json!({"ok": true, "team": "Acme", "user_id": "UBOT"})) }))
    .route("/api/apps.connections.open", post(move || async move { Json(json!({"ok": true, "url": format!("ws://{addr}/ws")})) }))
    .route("/api/users.info", get(|| async { Json(json!({"ok": true, "user": {"real_name": "Dana"}})) }))
    .route(
      "/api/chat.postMessage",
      post(move |Json(body): Json<Value>| {
        let posted = posted2.clone();
        async move {
          posted.lock().unwrap().push(body);
          Json(json!({"ok": true}))
        }
      }),
    )
    .route(
      "/ws",
      get(|ws: WebSocketUpgrade| async move {
        ws.on_upgrade(|mut socket| async move {
          let env = json!({"envelope_id": "e1", "type": "events_api", "payload": {"event": {"type": "message", "channel_type": "im", "channel": "D1", "user": "U1", "text": "write a plan", "ts": "1.1"}}});
          let _ = socket.send(AxMsg::Text(env.to_string().into())).await;
          while let Some(Ok(_)) = socket.recv().await {}
        })
      }),
    );
  tokio::spawn(async move { axum::serve(listener, slack).await.unwrap() });

  let dir = tempfile::tempdir().unwrap();
  let mut profile = provider::preset::build("lite-llm", "litellm");
  profile.endpoint = llm;
  let settings = config::Settings { providers: vec![profile], provider: "litellm".into(), model: "grok-4".into(), memory: false, ..Default::default() };
  let rt = agent::Runtime::start(dir.path().to_path_buf(), settings).await.unwrap();
  let (bot, _) = api::team::create(&rt).await.unwrap();
  let mut p = bot.profile();
  p.name = "Helper".into();
  p.description = "I answer the team's questions.".into();
  store::bots::update(&rt.pool, &bot.id, &p).await.unwrap();
  api::team::publish(&rt, &bot.id, true).await.unwrap();
  let base = format!("http://{addr}/api");
  // Keychain-backed tokens: skip if this machine's keychain is unavailable.
  if let Err(e) = api::slack::connect(&rt, &bot.id, "xoxb-test", "xapp-test", Some(&base)).await {
    eprintln!("skipping: {e}");
    return;
  }
  let got = tokio::time::timeout(Duration::from_secs(20), async {
    loop {
      tokio::time::sleep(Duration::from_millis(200)).await;
      if let Some(b) = posted.lock().unwrap().first().cloned() {
        return b;
      }
    }
  })
  .await
  .expect("no reply posted to Slack");
  assert_eq!(got["channel"], "D1");
  assert!(got["text"].as_str().unwrap().contains("Wrote the plan."));
  let chat = store::slack::chat_for(&rt.pool, &bot.id, "im:D1").await.unwrap().unwrap();
  let msgs = store::messages::list(&rt.pool, &chat).await.unwrap();
  assert!(msgs[0].body.starts_with("Dana (on Slack): write a plan"));
  api::slack::remove(&rt, &bot.id).await.unwrap();
}

/// Two Macs sharing a sync folder converge on the same sections.
#[tokio::test(flavor = "multi_thread")]
async fn sections_sync_between_devices() {
  let shared = tempfile::tempdir().unwrap();
  let (d1, d2) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
  let settings = config::Settings { sync_folder: shared.path().display().to_string(), memory: false, ..Default::default() };
  let a = agent::Runtime::start(d1.path().to_path_buf(), settings.clone()).await.unwrap();
  let b = agent::Runtime::start(d2.path().to_path_buf(), settings).await.unwrap();
  for rt in [&a, &b] {
    api::bots::create(rt, Some("Scout")).await.unwrap();
    api::bots::create(rt, Some("Writer")).await.unwrap();
  }
  // Mac A files Scout under "Hiring" and pins Writer.
  let s = store::sections::create(&a.pool, "Hiring").await.unwrap();
  let scout = store::bots::find(&a.pool, "Scout").await.unwrap().unwrap();
  store::bots::set_section(&a.pool, &scout.id, Some(&s.id)).await.unwrap();
  let writer = store::bots::find(&a.pool, "Writer").await.unwrap().unwrap();
  store::bots::pin(&a.pool, &writer.id, true).await.unwrap();
  // B syncs first (nothing of its own yet), then A pushes, then B pulls.
  api::sync::tick(&b).await.unwrap();
  tokio::time::sleep(Duration::from_millis(5)).await;
  api::sync::tick(&a).await.unwrap();
  api::sync::tick(&b).await.unwrap();
  let secs = store::sections::all(&b.pool).await.unwrap();
  assert_eq!(secs.len(), 1);
  assert_eq!(secs[0].name, "Hiring");
  let scout_b = store::bots::find(&b.pool, "Scout").await.unwrap().unwrap();
  assert_eq!(scout_b.section_id.as_deref(), Some(s.id.as_str()));
  assert!(store::bots::find(&b.pool, "Writer").await.unwrap().unwrap().pinned);
  // A rename on B flows back to A.
  store::sections::rename(&b.pool, &s.id, "Recruiting").await.unwrap();
  tokio::time::sleep(Duration::from_millis(5)).await;
  api::sync::tick(&b).await.unwrap();
  api::sync::tick(&a).await.unwrap();
  assert_eq!(store::sections::all(&a.pool).await.unwrap()[0].name, "Recruiting");
}

/// Recreate waits for a running job to reach its safe point, rebuilds the
/// computer, keeps files, and comes back ready. Needs Chrome/Chromium.
#[tokio::test(flavor = "multi_thread")]
async fn recreate_pauses_at_safe_point() {
  if computer::browser::find_chrome().is_none() {
    eprintln!("skipping: no Chromium-family browser");
    return;
  }
  let dir = tempfile::tempdir().unwrap();
  let settings = config::Settings { memory: false, ..Default::default() };
  let rt = agent::Runtime::start(dir.path().to_path_buf(), settings).await.unwrap();
  std::fs::write(rt.computer.workspace().join("keep.txt"), "durable").unwrap();
  api::computer::start(&rt).await.unwrap();
  // A "job" in progress holds the gate; recreate must wait for it.
  let job = rt.gate.read().await;
  let rt2 = rt.clone();
  let started = std::time::Instant::now();
  let rec = tokio::spawn(async move { api::computer::recreate(&rt2).await });
  tokio::time::sleep(Duration::from_millis(400)).await;
  assert!(!rec.is_finished(), "recreate must wait for the safe point");
  drop(job);
  rec.await.unwrap().unwrap();
  assert!(started.elapsed() >= Duration::from_millis(400));
  assert_eq!(std::fs::read_to_string(rt.computer.workspace().join("keep.txt")).unwrap(), "durable");
  assert!(rt.browser.running().await);
  assert!(!api::computer::snapshots(&rt).is_empty(), "recreate backs up first");
  // Idle hibernation stops the browser; the next screen use wakes it.
  assert!(api::computer::hibernate_after(&rt, 0).await);
  assert!(!rt.browser.running().await && api::computer::asleep());
  rt.browser.open("b1", "about:blank").await.unwrap();
  api::computer::wake_check(&rt).await;
  assert!(!api::computer::asleep());
  rt.browser.shutdown().await;
}

/// The API-token Jira connector, end to end: add it, connect (credentials
/// checked against a fake Jira), and call a real mcp-atlassian tool that
/// reads from that fake Jira. Needs uv; run with --ignored.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn jira_token_connector_end_to_end() {
  use axum::extract::Path;
  use std::sync::Mutex;
  let seen: Arc<Mutex<Vec<String>>> = Arc::default();
  let seen2 = seen.clone();
  let issue = |key: String| json!({
    "id": "10001", "key": key, "self": "x",
    "fields": { "summary": "Checkout fails on Safari", "description": "Steps: open cart", "status": { "name": "To Do", "statusCategory": { "name": "To Do" } },
      "issuetype": { "name": "Bug" }, "priority": { "name": "High" }, "created": "2026-09-30T10:00:00.000+0000", "updated": "2026-09-30T10:00:00.000+0000", "labels": [], "comment": { "comments": [] } }
  });
  let app = Router::new()
    .route("/rest/api/2/myself", get(|| async { Json(json!({ "displayName": "Pat Probe", "accountId": "abc" })) }))
    .route("/rest/api/3/myself", get(|| async { Json(json!({ "displayName": "Pat Probe", "accountId": "abc" })) }))
    .route("/rest/api/2/issue/{key}", get(move |Path(key): Path<String>| async move { Json(issue(key)) }))
    .route("/rest/api/3/issue/{key}", get(move |Path(key): Path<String>| async move { Json(issue(key)) }))
    .fallback(move |uri: axum::http::Uri| {
      let seen = seen2.clone();
      async move {
        seen.lock().unwrap().push(uri.to_string());
        Json(json!({}))
      }
    });
  let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = l.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(l, app).await.unwrap() });

  let dir = tempfile::tempdir().unwrap();
  let rt = agent::Runtime::start(dir.path().to_path_buf(), config::Settings { memory: false, ..Default::default() }).await.unwrap();
  let p = api::connect::add(&rt, "jiratoken").await.unwrap();
  let mut values = std::collections::HashMap::new();
  values.insert("JIRA_URL".to_string(), format!("http://{addr}"));
  values.insert("JIRA_USERNAME".to_string(), "probe@acme.com".to_string());
  values.insert("JIRA_API_TOKEN".to_string(), "good-token".to_string());
  values.insert("CONFLUENCE_URL".to_string(), String::new());
  if let Err(e) = api::connect::connect_fields(&rt, &p.id, "work", values).await {
    panic!("connect failed: {e}");
  }
  let offered = rt.plugins.offered(&rt).await;
  let get_issue = offered.iter().find(|o| o.tool.name == "jira_get_issue").expect("jira_get_issue offered");
  let (text, error) = rt.plugins.call(&rt, get_issue, json!({ "issue_key": "SHOP-42" })).await.unwrap();
  assert!(!error, "{text}\nunmatched: {:?}", seen.lock().unwrap());
  assert!(text.contains("Checkout fails on Safari"), "{text}");
  api::connect::remove(&rt, &p.id).await.unwrap();
}
