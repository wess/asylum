//! Socket Mode: open a WebSocket with the app-level token, acknowledge
//! each envelope, and hand Events API events to the caller. Reconnects
//! when Slack asks (or the socket drops) until stopped.

use crate::route::Event;
use anyhow::{bail, Result};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as Ws;

pub async fn open_url(base: &str, app_token: &str) -> Result<String> {
  let v: Value = reqwest::Client::new()
    .post(format!("{base}/apps.connections.open"))
    .bearer_auth(app_token)
    .send()
    .await?
    .json()
    .await?;
  if v["ok"].as_bool() != Some(true) {
    bail!("Slack apps.connections.open: {}", v["error"].as_str().unwrap_or("failed"));
  }
  Ok(v["url"].as_str().unwrap_or_default().to_string())
}

/// Pull the event out of an envelope (and its id to acknowledge).
pub fn envelope(text: &str) -> (Option<String>, Option<Event>, bool) {
  let Ok(v) = serde_json::from_str::<Value>(text) else { return (None, None, false) };
  let id = v["envelope_id"].as_str().map(str::to_string);
  let reconnect = v["type"] == "disconnect";
  let event = (v["type"] == "events_api")
    .then(|| serde_json::from_value::<Event>(v["payload"]["event"].clone()).ok())
    .flatten();
  (id, event, reconnect)
}

/// Run until the receiver for events is dropped.
pub async fn run(base: String, app_token: String, out: mpsc::UnboundedSender<Event>) -> Result<()> {
  let mut failures = 0u32;
  while !out.is_closed() {
    let url = match open_url(&base, &app_token).await {
      Ok(u) => u,
      Err(e) => {
        failures += 1;
        if failures > 5 {
          return Err(e);
        }
        tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(failures.min(5)))).await;
        continue;
      }
    };
    let Ok((ws, _)) = tokio_tungstenite::connect_async(url.as_str()).await else {
      failures += 1;
      tokio::time::sleep(std::time::Duration::from_secs(2)).await;
      continue;
    };
    failures = 0;
    let (mut sink, mut stream) = ws.split();
    while let Some(Ok(frame)) = stream.next().await {
      match frame {
        Ws::Text(t) => {
          let (id, event, reconnect) = envelope(&t);
          if let Some(id) = id {
            let _ = sink.send(Ws::Text(json!({ "envelope_id": id }).to_string().into())).await;
          }
          if let Some(e) = event {
            if out.send(e).is_err() {
              return Ok(());
            }
          }
          if reconnect {
            break;
          }
        }
        Ws::Ping(p) => {
          let _ = sink.send(Ws::Pong(p)).await;
        }
        Ws::Close(_) => break,
        _ => {}
      }
    }
  }
  Ok(())
}

#[cfg(test)]
#[path = "../tests/socket.rs"]
mod tests;
