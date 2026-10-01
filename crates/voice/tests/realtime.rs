use super::*;

#[test]
fn builds_url_and_session() {
  assert_eq!(url(""), "wss://api.x.ai/v1/realtime?model=grok-voice-latest");
  let s = session_update(&Config { voice: "Ara".into(), speed: 1.2, language: "es".into(), ..Default::default() });
  assert_eq!(s["session"]["voice"], "ara");
  assert_eq!(s["session"]["audio"]["output"]["speed"], 1.2f32 as f64);
  assert_eq!(s["session"]["audio"]["input"]["transcription"]["language_hint"], "es");
  assert_eq!(s["session"]["turn_detection"]["type"], "server_vad");
}

use axum::extract::ws::{Message as AxMsg, WebSocketUpgrade};
use axum::routing::get;
use axum::Router;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct Sink {
  played: Arc<Mutex<usize>>,
  cleared: Arc<Mutex<usize>>,
}

impl Output for Sink {
  fn push(&self, samples: &[i16]) {
    *self.played.lock().unwrap() += samples.len();
  }

  fn clear(&self) {
    *self.cleared.lock().unwrap() += 1;
  }
}

/// A fake realtime server: checks the session setup and streamed audio,
/// then answers with transcripts, audio, a barge-in, and a tool call.
#[tokio::test(flavor = "multi_thread")]
async fn realtime_session_round_trip() {
  let (seen_tx, mut seen_rx) = mpsc::unbounded_channel::<Value>();
  let app = Router::new().route(
    "/v1/realtime",
    get(move |ws: WebSocketUpgrade| {
      let seen_tx = seen_tx.clone();
      async move {
        ws.on_upgrade(move |mut socket| async move {
          // session.update, then at least one audio append.
          for _ in 0..2 {
            if let Some(Ok(AxMsg::Text(t))) = socket.recv().await {
              let _ = seen_tx.send(serde_json::from_str(&t).unwrap());
            }
          }
          let audio = base64::engine::general_purpose::STANDARD.encode(pcm::le_bytes(&[1i16; 480]));
          let events = [
            json!({"type": "conversation.item.input_audio_transcription.completed", "transcript": "What's on my calendar?"}),
            json!({"type": "response.output_audio.delta", "delta": audio}),
            json!({"type": "response.output_audio_transcript.delta", "delta": "You have two meetings."}),
            json!({"type": "response.done"}),
            json!({"type": "input_audio_buffer.speech_started"}),
            json!({"type": "response.function_call_arguments.done", "call_id": "c1", "name": "note_task", "arguments": "{\"task\":\"prep notes\"}"}),
          ];
          for e in events {
            let _ = socket.send(AxMsg::Text(e.to_string().into())).await;
          }
          // The tool result comes back, then a response.create.
          for _ in 0..2 {
            if let Some(Ok(AxMsg::Text(t))) = socket.recv().await {
              let _ = seen_tx.send(serde_json::from_str(&t).unwrap());
            }
          }
          let _ = socket.send(AxMsg::Close(None)).await;
        })
      }
    }),
  );
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = listener.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let (mic_tx, mic_rx) = mpsc::unbounded_channel();
  let sink = Sink::default();
  let cfg = Config { instructions: "Be brief.".into(), voice: "Eve".into(), ..Default::default() };
  let mut session = connect_with("key", &format!("http://{addr}/v1"), cfg, mic_rx, sink.clone()).await.unwrap();
  mic_tx.send(vec![0.1; 2400]).unwrap();

  let setup = seen_rx.recv().await.unwrap();
  assert_eq!(setup["type"], "session.update");
  assert_eq!(setup["session"]["instructions"], "Be brief.");
  assert_eq!(seen_rx.recv().await.unwrap()["type"], "input_audio_buffer.append");

  let mut got = Vec::new();
  while let Some(u) = tokio::time::timeout(std::time::Duration::from_secs(5), session.updates.recv()).await.unwrap() {
    if let Update::Tool { call, .. } = &u {
      session.tool_result(call, "{\"ok\":true}");
    }
    let closed = u == Update::Closed;
    got.push(u);
    if closed {
      break;
    }
  }
  assert!(got.contains(&Update::UserDone("What's on my calendar?".into())));
  assert!(got.contains(&Update::Assistant("You have two meetings.".into())));
  assert!(got.contains(&Update::AssistantDone));
  assert!(got.iter().any(|u| matches!(u, Update::Tool { name, .. } if name == "note_task")));
  assert_eq!(*sink.played.lock().unwrap(), 480);
  assert_eq!(*sink.cleared.lock().unwrap(), 1, "barge-in should clear playback");
  assert_eq!(seen_rx.recv().await.unwrap()["item"]["type"], "function_call_output");
  assert_eq!(seen_rx.recv().await.unwrap()["type"], "response.create");
}
