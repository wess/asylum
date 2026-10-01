//! A live voice chat over xAI's realtime API: microphone audio streams up,
//! the model's speech streams back to the speaker, and both sides'
//! transcripts arrive as updates. Server voice-activity detection takes
//! turns; when the user starts talking, queued playback is dropped.

use crate::mic::Mic;
use crate::speaker::Speaker;
use crate::{pcm, RATE};
use anyhow::Result;
use base64::Engine;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message as Ws;

pub const MODEL: &str = "grok-voice-latest";

#[derive(Clone, Debug, Default)]
pub struct Config {
  pub instructions: String,
  pub voice: String,
  pub speed: f32,
  pub language: String,
  pub device: String,
  /// Function tools (realtime shape: {type, name, description, parameters}).
  pub tools: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Update {
  Connected,
  /// The user's current utterance so far.
  User(String),
  /// The user finished an utterance.
  UserDone(String),
  /// More of the assistant's reply.
  Assistant(String),
  AssistantDone,
  Speaking(bool),
  Level(f32),
  Tool { call: String, name: String, args: String },
  Error(String),
  Closed,
}

enum Cmd {
  Configure(Value),
  Mute(bool),
  Hangup,
  ToolResult(String, String),
  Say(String),
}

pub struct Session {
  tx: mpsc::UnboundedSender<Cmd>,
  pub updates: mpsc::UnboundedReceiver<Update>,
}

/// Controls for a live session, separable from its update stream.
#[derive(Clone)]
pub struct Handle {
  tx: mpsc::UnboundedSender<Cmd>,
}

impl Session {
  pub fn split(self) -> (Handle, mpsc::UnboundedReceiver<Update>) {
    (Handle { tx: self.tx }, self.updates)
  }
}

impl Handle {
  pub fn mute(&self, on: bool) {
    let _ = self.tx.send(Cmd::Mute(on));
  }

  pub fn hangup(&self) {
    let _ = self.tx.send(Cmd::Hangup);
  }

  pub fn tool_result(&self, call: &str, output: &str) {
    let _ = self.tx.send(Cmd::ToolResult(call.into(), output.into()));
  }

  /// Change voice, speed, or language mid-call.
  pub fn configure(&self, cfg: &Config) {
    let _ = self.tx.send(Cmd::Configure(session_update(cfg)));
  }
}

impl Session {
  pub fn mute(&self, on: bool) {
    let _ = self.tx.send(Cmd::Mute(on));
  }

  pub fn hangup(&self) {
    let _ = self.tx.send(Cmd::Hangup);
  }

  pub fn tool_result(&self, call: &str, output: &str) {
    let _ = self.tx.send(Cmd::ToolResult(call.into(), output.into()));
  }

  /// Send a typed message into the call.
  pub fn say(&self, text: &str) {
    let _ = self.tx.send(Cmd::Say(text.into()));
  }
}

pub fn url(base: &str) -> String {
  let b = crate::base(base);
  let ws = b.replacen("https://", "wss://", 1).replacen("http://", "ws://", 1);
  format!("{ws}/realtime?model={MODEL}")
}

pub fn session_update(c: &Config) -> Value {
  let mut s = json!({
    "type": "session.update",
    "session": {
      "instructions": c.instructions,
      "voice": if c.voice.is_empty() { "eve".to_string() } else { c.voice.to_lowercase() },
      "turn_detection": { "type": "server_vad" },
      "audio": {
        "input": { "format": { "type": "audio/pcm", "rate": RATE } },
        "output": { "format": { "type": "audio/pcm", "rate": RATE }, "speed": if c.speed > 0.0 { c.speed } else { 1.0 } }
      },
      "tools": c.tools,
    }
  });
  if !c.language.is_empty() && c.language != "auto" {
    s["session"]["audio"]["input"]["transcription"] = json!({ "language_hint": c.language });
  }
  s
}

/// Where the session's audio goes: playback and barge-in.
pub trait Output: Send + 'static {
  fn push(&self, samples: &[i16]);
  fn clear(&self);
  fn stop(&self) {}
}

impl Output for Speaker {
  fn push(&self, samples: &[i16]) {
    Speaker::push(self, samples)
  }

  fn clear(&self) {
    Speaker::clear(self)
  }

  fn stop(&self) {
    Speaker::stop(self)
  }
}

/// A live call on the real microphone and speaker.
pub async fn connect(key: &str, base: &str, cfg: Config) -> Result<Session> {
  let mut mic = Mic::start(&cfg.device)?;
  let speaker = Speaker::start()?;
  // The task owns the microphone, so it records for as long as the call runs.
  let (keep_tx, keep_rx) = mpsc::unbounded_channel::<Vec<f32>>();
  tokio::spawn(async move {
    while let Some(chunk) = mic.rx.recv().await {
      if keep_tx.send(chunk).is_err() {
        break;
      }
    }
  });
  connect_with(key, base, cfg, keep_rx, speaker).await
}

/// A call with audio supplied by the caller: `mic` delivers 24 kHz mono
/// samples, `out` plays the model's speech.
pub async fn connect_with<O: Output>(key: &str, base: &str, cfg: Config, mut mic: mpsc::UnboundedReceiver<Vec<f32>>, out: O) -> Result<Session> {
  let mut req = url(base).into_client_request()?;
  req.headers_mut().insert("Authorization", format!("Bearer {key}").parse()?);
  let (ws, _) = tokio_tungstenite::connect_async(req).await?;
  let (mut sink, mut stream) = ws.split();
  sink.send(Ws::Text(session_update(&cfg).to_string().into())).await?;

  let (tx, mut cmds) = mpsc::unbounded_channel();
  let (up, updates) = mpsc::unbounded_channel();
  let _ = up.send(Update::Connected);

  tokio::spawn(async move {
    let mut muted = false;
    let mut user = String::new();
    loop {
      tokio::select! {
        chunk = mic.recv() => {
          let Some(samples) = chunk else { break };
          let _ = up.send(Update::Level(if muted { 0.0 } else { pcm::level(&samples) }));
          if muted { continue; }
          let bytes = pcm::le_bytes(&pcm::to_i16(&samples));
          let msg = json!({ "type": "input_audio_buffer.append", "audio": base64::engine::general_purpose::STANDARD.encode(bytes) });
          if sink.send(Ws::Text(msg.to_string().into())).await.is_err() { break; }
        }
        cmd = cmds.recv() => match cmd {
          Some(Cmd::Mute(m)) => muted = m,
          Some(Cmd::Configure(v)) => { let _ = sink.send(Ws::Text(v.to_string().into())).await; }
          Some(Cmd::ToolResult(call, output)) => {
            let item = json!({ "type": "conversation.item.create", "item": { "type": "function_call_output", "call_id": call, "output": output } });
            let _ = sink.send(Ws::Text(item.to_string().into())).await;
            let _ = sink.send(Ws::Text(json!({ "type": "response.create" }).to_string().into())).await;
          }
          Some(Cmd::Say(text)) => {
            let item = json!({ "type": "conversation.item.create", "item": { "type": "message", "role": "user", "content": [{ "type": "input_text", "text": text }] } });
            let _ = sink.send(Ws::Text(item.to_string().into())).await;
            let _ = sink.send(Ws::Text(json!({ "type": "response.create" }).to_string().into())).await;
          }
          Some(Cmd::Hangup) | None => { let _ = sink.close().await; break; }
        },
        frame = stream.next() => {
          let Some(Ok(frame)) = frame else {
            let _ = up.send(Update::Error("The call ended unexpectedly.".into()));
            break;
          };
          let v: Value = match frame {
            Ws::Text(t) => serde_json::from_str(&t).unwrap_or(Value::Null),
            Ws::Binary(b) => { out.push(&pcm::from_le_bytes(&b)); continue; }
            Ws::Close(_) => break,
            _ => continue,
          };
          handle(&v, &out, &up, &mut user);
        }
      }
    }
    out.stop();
    let _ = up.send(Update::Closed);
  });
  Ok(Session { tx, updates })
}

fn handle<O: Output>(v: &Value, speaker: &O, up: &mpsc::UnboundedSender<Update>, user: &mut String) {
  let kind = v["type"].as_str().unwrap_or("");
  match kind {
    "response.output_audio.delta" | "response.audio.delta" => {
      if let Some(b64) = v["delta"].as_str() {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
          speaker.push(&pcm::from_le_bytes(&bytes));
          let _ = up.send(Update::Speaking(true));
        }
      }
    }
    "input_audio_buffer.speech_started" => {
      speaker.clear();
      let _ = up.send(Update::Speaking(false));
    }
    "conversation.item.input_audio_transcription.updated" | "conversation.item.input_audio_transcription.delta" => {
      if let Some(t) = v["transcript"].as_str() {
        *user = t.to_string();
      } else if let Some(d) = v["delta"].as_str() {
        user.push_str(d);
      }
      let _ = up.send(Update::User(user.clone()));
    }
    "conversation.item.input_audio_transcription.completed" => {
      let t = v["transcript"].as_str().map(str::to_string).unwrap_or_else(|| user.clone());
      user.clear();
      let _ = up.send(Update::UserDone(t));
    }
    "response.audio_transcript.delta" | "response.output_audio_transcript.delta" => {
      if let Some(d) = v["delta"].as_str() {
        let _ = up.send(Update::Assistant(d.to_string()));
      }
    }
    "response.done" => {
      let _ = up.send(Update::AssistantDone);
    }
    "response.function_call_arguments.done" => {
      let _ = up.send(Update::Tool {
        call: v["call_id"].as_str().unwrap_or("").into(),
        name: v["name"].as_str().unwrap_or("").into(),
        args: v["arguments"].as_str().unwrap_or("{}").into(),
      });
    }
    "error" => {
      let msg = v["error"]["message"].as_str().or(v["message"].as_str()).unwrap_or("The realtime connection failed.");
      let _ = up.send(Update::Error(msg.to_string()));
    }
    _ => {}
  }
}

#[cfg(test)]
#[path = "../tests/realtime.rs"]
mod tests;
