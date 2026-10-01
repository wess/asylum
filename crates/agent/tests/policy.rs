use super::*;
use config::LocalExec;

async fn rt() -> (Runtime, tempfile::TempDir) {
  let dir = tempfile::tempdir().unwrap();
  let settings = config::Settings { memory: false, local_exec: LocalExec::Always, auto_review: false, ..Default::default() };
  (Runtime::start(dir.path().to_path_buf(), settings).await.unwrap(), dir)
}

#[tokio::test(flavor = "multi_thread")]
async fn policy_locks_rules_requires_plugins_and_caps() {
  let (rt, dir) = rt().await;
  let file = dir.path().join("policy.json");
  std::fs::write(&file, r#"{
    "organization": "Acme",
    "disabled-plugins": ["stripe"],
    "required-plugins": ["deepwiki"],
    "local-exec": "never",
    "auto-review": true,
    "templates": "team",
    "rules": [{"kind": "ask", "text": "Before emailing outside acme.com"}]
  }"#).unwrap();
  apply(&rt, &file).await.unwrap();
  let p = rt.policy();
  assert_eq!(p.organization, "Acme");
  assert_eq!(p.cap_local(rt.settings().local_exec), LocalExec::Never);
  assert!(p.auto_review(rt.settings().auto_review));

  // Locked rule present and not editable or deletable.
  let rules = store::rules::all(&rt.pool).await.unwrap();
  let locked: Vec<_> = rules.iter().filter(|r| r.locked).collect();
  assert_eq!(locked.len(), 1);
  assert!(store::rules::delete(&rt.pool, &locked[0].id).await.is_err());

  // Required plugin added and can't be removed; disabled one can't be added.
  let dw = store::plugins::by_catalog(&rt.pool, "deepwiki").await.unwrap().expect("required plugin added");
  assert!(crate::api::connect::remove(&rt, &dw.id).await.is_err());
  assert!(crate::api::connect::add(&rt, "stripe").await.is_err());

  // Templates: team-only allowed, public refused.
  let (b, _) = crate::api::bots::create(&rt, Some("Scout")).await.unwrap();
  assert!(crate::template::share(&rt, &b.id, "public").await.is_err());
  assert!(crate::template::share(&rt, &b.id, "team").await.is_ok());

  // Local execution is denied under the ceiling.
  let a = crate::approve::Action { bot: &b.id, chat: "c", run: "r", tool: "run_local", target: "ls", args: "{}", class: crate::approve::Class::Local, interactive: true, request: "", profile: "" };
  assert!(matches!(crate::approve::check(&rt, &a).await.unwrap(), crate::approve::Verdict::Deny(_)));

  // Policy changed: rule replaced; a broken file keeps the last good policy.
  std::fs::write(&file, r#"{"organization": "Acme", "rules": [{"kind": "allow", "text": "Reading public web pages"}]}"#).unwrap();
  apply(&rt, &file).await.unwrap();
  let locked: Vec<_> = store::rules::all(&rt.pool).await.unwrap().into_iter().filter(|r| r.locked).collect();
  assert_eq!(locked.len(), 1);
  assert_eq!(locked[0].kind, "allow");
  std::fs::write(&file, "{ broken").unwrap();
  apply(&rt, &file).await.unwrap();
  assert_eq!(rt.policy().organization, "Acme");
  assert!(rt.policy_error.read().unwrap().is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn policy_adds_required_team_bots_and_can_disable_team_bots() {
  let (rt, dir) = rt().await;
  // An owner's published Team Bot link.
  let (owner, _) = crate::api::team::create(&rt).await.unwrap();
  let mut p = owner.profile();
  p.name = "Helpdesk".into();
  p.description = "Answers IT questions.".into();
  store::bots::update(&rt.pool, &owner.id, &p).await.unwrap();
  let link = crate::api::team::link(&rt, &owner.id).await.unwrap();

  let file = dir.path().join("policy.json");
  std::fs::write(&file, serde_json::json!({ "team-bots": [link] }).to_string()).unwrap();
  apply(&rt, &file).await.unwrap();
  apply(&rt, &file).await.unwrap(); // idempotent: added once
  let added: Vec<_> = store::bots::list(&rt.pool).await.unwrap().into_iter().filter(|b| b.required).collect();
  assert_eq!(added.len(), 1);
  assert!(crate::api::bots::delete(&rt, &added[0].id).await.is_err());

  // Dropped from policy: no longer required.
  std::fs::write(&file, r#"{"team-bots-enabled": false}"#).unwrap();
  apply(&rt, &file).await.unwrap();
  assert!(!store::bots::get(&rt.pool, &added[0].id).await.unwrap().required);
  let err = crate::api::team::create(&rt).await.unwrap_err().to_string();
  assert!(err.contains(crate::api::team::NOT_AVAILABLE));
  let err = crate::api::team::join(&rt, "asylum://template/garbage").await.unwrap_err().to_string();
  assert!(err.contains(crate::api::team::NOT_AVAILABLE));
}

#[tokio::test(flavor = "multi_thread")]
async fn network_controls_gate_commands_and_web_tools() {
  if !computer::sandbox::available() {
    return;
  }
  let (rt, dir) = rt().await;
  let file = dir.path().join("policy.json");
  let run = |cmd: &'static str| {
    let rt = rt.clone();
    async move {
      let ws = rt.computer.workspace();
      computer::shell::run(computer::shell::Spec { command: cmd, cwd: &ws, place: computer::shell::Place::Computer, env: &[], timeout: std::time::Duration::from_secs(20), net: rt.net() }).await.unwrap()
    }
  };
  // Allowlist: a host off the list gets the proxy's 403, never the network.
  std::fs::write(&file, r#"{"network": {"mode": "allowlist", "allowed": ["acme.com"]}}"#).unwrap();
  apply(&rt, &file).await.unwrap();
  let out = run("curl -s -o /dev/null -w '%{http_code}' http://blocked.example/").await;
  assert_eq!(out.stdout.trim(), "403", "{}", out.render());
  // Bypassing the proxy is refused by the sandbox.
  let out = run("curl -s --noproxy '*' --max-time 5 http://93.184.215.14/ -o /dev/null -w '%{http_code}'; echo \" exit=$?\"").await;
  assert!(!out.stdout.contains("200"), "{}", out.render());
  let err = computer::web::fetch("http://blocked.example/", rt.web_proxy()).await.unwrap_err().to_string();
  assert!(err.contains("403"), "{err}");
  // Offline: nothing gets out.
  std::fs::write(&file, r#"{"network": {"mode": "offline"}}"#).unwrap();
  apply(&rt, &file).await.unwrap();
  let out = run("curl -s --max-time 5 http://acme.com/ -o /dev/null -w '%{http_code}'").await;
  assert!(!out.stdout.contains("200"), "{}", out.render());
  // Back to open.
  std::fs::write(&file, "{}").unwrap();
  apply(&rt, &file).await.unwrap();
  assert_eq!(rt.net(), computer::sandbox::Net::Open);
}

#[tokio::test(flavor = "multi_thread")]
async fn setup_and_check_scripts_and_cloud_agents() {
  let (rt, dir) = rt().await;
  let file = dir.path().join("policy.json");
  std::fs::write(&file, r#"{"setup-script": "echo ready > setup.txt", "check-script": "test -f missing.txt", "cloud-agents": false}"#).unwrap();
  apply(&rt, &file).await.unwrap();
  crate::api::computer::setup(&rt).await;
  assert_eq!(std::fs::read_to_string(rt.computer.workspace().join("setup.txt")).unwrap().trim(), "ready");
  // The failing check is reported.
  let notes = store::notifications::list(&rt.pool, 10).await.unwrap();
  assert!(notes.iter().any(|n| n.title == "Check script failed"));
  // Setup runs once per script.
  std::fs::remove_file(rt.computer.workspace().join("setup.txt")).unwrap();
  crate::api::computer::setup(&rt).await;
  assert!(!rt.computer.workspace().join("setup.txt").exists());
  // Cloud Agents off: a Bot pinned to a CLI provider is refused.
  let mut s = rt.settings();
  s.providers = vec![provider::preset::build("codex", "codex")];
  s.provider = "codex".into();
  s.model = "gpt-5".into();
  rt.set_settings(s);
  let err = rt.provider(None).await.err().expect("refused").to_string();
  assert!(err.contains("Cloud Agents"), "{err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn export_has_prompts_responses_and_tool_io() {
  let (rt, _dir) = rt().await;
  let (bot, chat) = crate::api::bots::create(&rt, Some("Scout")).await.unwrap();
  use store::messages::{self, New};
  messages::add(&rt.pool, New { chat: &chat.id, bot: None, role: messages::USER, body: "find rust news", parts: &[], status: messages::DONE, run: None, thread: None }).await.unwrap();
  let parts = [serde_json::json!({"type": "tool", "id": "c1", "name": "web_search", "args": {"query": "rust"}, "result": "1. Rust 2.0", "status": "done"})];
  messages::add(&rt.pool, New { chat: &chat.id, bot: Some(&bot.id), role: messages::BOT, body: "Here's the news.", parts: &parts, status: messages::DONE, run: None, thread: None }).await.unwrap();
  let v = crate::api::export::chat_json(&rt, &chat.id).await.unwrap();
  let msgs = v["messages"].as_array().unwrap();
  assert_eq!(msgs[0]["text"], "find rust news");
  assert_eq!(msgs[1]["author"], "Scout");
  assert_eq!(msgs[1]["tools"][0]["input"]["query"], "rust");
  let md = crate::api::export::markdown(&v);
  assert!(md.contains("Here's the news.") && md.contains("web_search") && md.contains("Rust 2.0"));
}
