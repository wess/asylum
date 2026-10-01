use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn admin_api_requires_token_and_serves_data() {
  let dir = tempfile::tempdir().unwrap();
  let settings = config::Settings { memory: false, ..Default::default() };
  let rt = Runtime::start(dir.path().to_path_buf(), settings).await.unwrap();
  let (bot, chat) = crate::api::bots::create(&rt, Some("Scout")).await.unwrap();
  crate::audit::action(&rt, &bot.id, &chat.id, "web_search", "rust", "done", 12).await;
  crate::audit::change(&rt, "user", "settings.changed", "theme", "").await;

  let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
  let base = format!("http://{}", listener.local_addr().unwrap());
  let app = router(rt.clone(), "secret-token".into());
  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  let http = reqwest::Client::new();

  let r = http.get(format!("{base}/v1/agents")).send().await.unwrap();
  assert_eq!(r.status(), 401);
  let r = http.get(format!("{base}/v1/agents")).bearer_auth("wrong-token!").send().await.unwrap();
  assert_eq!(r.status(), 401);

  let get = |path: &str| http.get(format!("{base}{path}")).bearer_auth("secret-token").send();
  let bots: Value = get("/v1/agents").await.unwrap().json().await.unwrap();
  assert_eq!(bots[0]["name"], "Scout");
  let actions: Value = get("/v1/actions").await.unwrap().json().await.unwrap();
  assert_eq!(actions[0]["name"], "web_search");
  assert_eq!(actions[0]["outcome"], "done");
  let audit: Value = get("/v1/audit").await.unwrap().json().await.unwrap();
  assert_eq!(audit[0]["name"], "settings.changed");
  let ins: Value = get("/v1/insights").await.unwrap().json().await.unwrap();
  assert_eq!(ins["actions"], 1);
  assert_eq!(ins["tools"][0][0], "web_search");
  let exp: Value = get(&format!("/v1/chats/{}/export", chat.id)).await.unwrap().json().await.unwrap();
  assert_eq!(exp["id"], chat.id);
  let r = http.post(format!("{base}/v1/chats/{}/messages", chat.id)).bearer_auth("secret-token").json(&json!({"text": ""})).send().await.unwrap();
  assert_eq!(r.status(), 400);
}

#[tokio::test(flavor = "multi_thread")]
async fn repeated_changes_are_logged_once() {
  let dir = tempfile::tempdir().unwrap();
  let rt = Runtime::start(dir.path().to_path_buf(), config::Settings { memory: false, ..Default::default() }).await.unwrap();
  for _ in 0..4 {
    crate::audit::change(&rt, "user", "settings.changed", "user-name", "").await;
  }
  crate::audit::change(&rt, "user", "settings.changed", "theme", "").await;
  let all = store::events::list(&rt.pool, store::events::AUDIT, 0, 100).await.unwrap();
  assert_eq!(all.len(), 2);
}
