//! Text to speech: POST /v1/tts, and the voice list.

use anyhow::{bail, Result};
use base64::Engine;
use serde::Deserialize;
use serde_json::json;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Voice {
  pub voice_id: String,
  #[serde(default)]
  pub name: String,
  #[serde(default)]
  pub language: String,
}

pub async fn voices(key: &str, base: &str) -> Result<Vec<Voice>> {
  let v: serde_json::Value = reqwest::Client::new()
    .get(format!("{}/tts/voices", crate::base(base)))
    .bearer_auth(key)
    .send()
    .await?
    .error_for_status()?
    .json()
    .await?;
  Ok(serde_json::from_value(v["voices"].clone()).unwrap_or_default())
}

/// MP3 bytes for `text`.
pub async fn speak(key: &str, base: &str, text: &str, voice: &str, language: &str, speed: f32) -> Result<Vec<u8>> {
  let lang = if language.is_empty() || language == "auto" || language == "system" { "en" } else { language };
  let res = reqwest::Client::new()
    .post(format!("{}/tts", crate::base(base)))
    .bearer_auth(key)
    .json(&json!({
      "text": text,
      "voice_id": if voice.is_empty() { "eve" } else { voice },
      "language": lang,
      "speed": speed,
      "output_format": { "codec": "mp3" }
    }))
    .send()
    .await?;
  if !res.status().is_success() {
    let body = res.text().await.unwrap_or_default();
    bail!("speech failed: {}", body.chars().take(200).collect::<String>());
  }
  let v: serde_json::Value = res.json().await?;
  let b64 = v["audio"].as_str().unwrap_or_default();
  Ok(base64::engine::general_purpose::STANDARD.decode(b64)?)
}
