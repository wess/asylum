use super::*;
use axum::extract::ws::{Message as AxMsg, WebSocketUpgrade};
use axum::routing::{get, post};
use axum::{Json, Router};

#[test]
fn parses_envelopes() {
  let t = r#"{"envelope_id":"e1","type":"events_api","payload":{"event":{"type":"app_mention","channel":"C1","user":"U1","text":"hi","ts":"1.0"}}}"#;
  let (id, e, rc) = envelope(t);
  assert_eq!(id.as_deref(), Some("e1"));
  assert_eq!(e.unwrap().kind, "app_mention");
  assert!(!rc);
  assert!(envelope(r#"{"type":"disconnect"}"#).2);
}

/// A fake Slack: apps.connections.open hands out a socket URL; the socket
/// sends one event and checks it gets acknowledged.
#[tokio::test(flavor = "multi_thread")]
async fn socket_mode_round_trip() {
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = listener.local_addr().unwrap();
  let (ack_tx, mut ack_rx) = mpsc::unbounded_channel::<String>();
  let app = Router::new()
    .route("/api/apps.connections.open", post(move || async move { Json(json!({"ok": true, "url": format!("ws://{addr}/ws")})) }))
    .route(
      "/ws",
      get(move |ws: WebSocketUpgrade| {
        let ack_tx = ack_tx.clone();
        async move {
          ws.on_upgrade(move |mut socket| async move {
            let env = json!({"envelope_id": "env-1", "type": "events_api", "payload": {"event": {"type": "message", "channel_type": "im", "channel": "D1", "user": "U1", "text": "hello", "ts": "1.1"}}});
            let _ = socket.send(AxMsg::Text(env.to_string().into())).await;
            if let Some(Ok(AxMsg::Text(t))) = socket.recv().await {
              let _ = ack_tx.send(t.to_string());
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
          })
        }
      }),
    );
  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  let (tx, mut rx) = mpsc::unbounded_channel();
  let base = format!("http://{addr}/api");
  tokio::spawn(run(base, "xapp-test".into(), tx));
  let e = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await.unwrap().unwrap();
  assert_eq!(e.text, "hello");
  let ack = tokio::time::timeout(std::time::Duration::from_secs(5), ack_rx.recv()).await.unwrap().unwrap();
  assert!(ack.contains("env-1"));
  drop(rx);
}
