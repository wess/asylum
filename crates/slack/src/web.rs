//! The few Slack Web API calls a Team Bot makes.

use anyhow::{bail, Result};
use serde_json::{json, Value};

async fn call(base: &str, token: &str, method: &str, body: Value) -> Result<Value> {
  let v: Value = reqwest::Client::new()
    .post(format!("{base}/{method}"))
    .bearer_auth(token)
    .json(&body)
    .send()
    .await?
    .json()
    .await?;
  if v["ok"].as_bool() != Some(true) {
    bail!("Slack {method}: {}", v["error"].as_str().unwrap_or("failed"));
  }
  Ok(v)
}

/// The bot's own identity: (team name, bot user ID).
pub async fn whoami(base: &str, bot_token: &str) -> Result<(String, String)> {
  let v = call(base, bot_token, "auth.test", json!({})).await?;
  Ok((v["team"].as_str().unwrap_or_default().into(), v["user_id"].as_str().unwrap_or_default().into()))
}

pub async fn post(base: &str, bot_token: &str, channel: &str, text: &str, thread: Option<&str>) -> Result<()> {
  let mut body = json!({ "channel": channel, "text": text });
  if let Some(t) = thread {
    body["thread_ts"] = json!(t);
  }
  call(base, bot_token, "chat.postMessage", body).await.map(|_| ())
}

/// A message only one person sees.
pub async fn ephemeral(base: &str, bot_token: &str, channel: &str, user: &str, text: &str) -> Result<()> {
  call(base, bot_token, "chat.postEphemeral", json!({ "channel": channel, "user": user, "text": text })).await.map(|_| ())
}

pub async fn user_name(base: &str, bot_token: &str, user: &str) -> String {
  match reqwest::Client::new()
    .get(format!("{base}/users.info?user={}", urlencoding::encode(user)))
    .bearer_auth(bot_token)
    .send()
    .await
  {
    Ok(r) => r
      .json::<Value>()
      .await
      .ok()
      .and_then(|v| v["user"]["real_name"].as_str().or(v["user"]["name"].as_str()).map(str::to_string))
      .unwrap_or_else(|| user.to_string()),
    Err(_) => user.to_string(),
  }
}
