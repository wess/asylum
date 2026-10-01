//! Speech to text: POST /v1/stt with a WAV file.

use anyhow::{bail, Result};

pub async fn transcribe(key: &str, base: &str, wav: Vec<u8>, language: &str) -> Result<String> {
  let part = reqwest::multipart::Part::bytes(wav).file_name("speech.wav").mime_str("audio/wav")?;
  let mut form = reqwest::multipart::Form::new().part("file", part);
  if !language.is_empty() && language != "auto" && language != "system" {
    form = form.text("language", language.to_string());
  }
  let res = reqwest::Client::new()
    .post(format!("{}/stt", crate::base(base)))
    .bearer_auth(key)
    .multipart(form)
    .send()
    .await?;
  if !res.status().is_success() {
    let body = res.text().await.unwrap_or_default();
    bail!("transcription failed: {}", body.chars().take(200).collect::<String>());
  }
  let v: serde_json::Value = res.json().await?;
  Ok(v["text"].as_str().unwrap_or_default().trim().to_string())
}
