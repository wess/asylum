//! Images from the xAI image API and speech from the system voice.

use crate::runtime::Runtime;
use anyhow::{anyhow, bail, Result};
use base64::Engine;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const IMAGE_MODELS: [&str; 3] = ["grok-2-image", "grok-2-image-1212", "grok-imagine-image"];

pub async fn image(rt: &Runtime, prompt: &str) -> Result<Vec<u8>> {
  let key = config::secret::xai_key().ok_or_else(|| anyhow!("Image generation uses xAI. Add your xAI API key in Settings → Providers."))?;
  let base = chat::DEFAULT_BASE.to_string();
  let models = rt.models.read().map(|m| m.clone()).unwrap_or_default();
  let model = IMAGE_MODELS
    .iter()
    .find(|m| models.iter().any(|a| a == *m))
    .copied()
    .or_else(|| models.iter().find(|m| m.contains("image")).map(|s| s.as_str()))
    .unwrap_or(IMAGE_MODELS[0])
    .to_string();
  let res = reqwest::Client::new()
    .post(format!("{base}/images/generations"))
    .bearer_auth(key)
    .json(&json!({ "model": model, "prompt": prompt, "n": 1, "response_format": "b64_json" }))
    .send()
    .await?;
  if !res.status().is_success() {
    let body = res.text().await.unwrap_or_default();
    bail!("image generation failed: {}", body.chars().take(300).collect::<String>());
  }
  let v: Value = res.json().await?;
  if let Some(b64) = v["data"][0]["b64_json"].as_str() {
    return Ok(base64::engine::general_purpose::STANDARD.decode(b64)?);
  }
  if let Some(url) = v["data"][0]["url"].as_str() {
    return Ok(reqwest::get(url).await?.bytes().await?.to_vec());
  }
  bail!("the image API returned no image")
}

/// Speak `text` to an audio file in the workspace: xAI text-to-speech,
/// falling back to the system voice.
pub async fn speak(rt: &Runtime, workspace: &Path, text: &str, voice: &str) -> Result<PathBuf> {
  let dir = workspace.join("voice");
  std::fs::create_dir_all(&dir)?;
  let s = rt.settings();
  if let Some(key) = config::secret::xai_key() {
    if let Ok(mp3) = voice::tts::speak(&key, "", text, voice, &s.voice_language, s.voice_speed).await {
      let path = dir.join(format!("{}.mp3", store::now()));
      std::fs::write(&path, mp3)?;
      return Ok(path);
    }
  }
  let path = dir.join(format!("{}.m4a", store::now()));
  let mut cmd = tokio::process::Command::new("say");
  if !voice.is_empty() && installed_voice(voice).await {
    cmd.arg("-v").arg(voice);
  }
  let status = cmd
    .arg("--file-format=m4af")
    .arg("--data-format=aac")
    .arg("-o")
    .arg(&path)
    .arg(text)
    .status()
    .await?;
  if !status.success() {
    bail!("could not synthesize speech");
  }
  Ok(path)
}

async fn installed_voice(name: &str) -> bool {
  let Ok(out) = tokio::process::Command::new("say").arg("-v").arg("?").output().await else { return false };
  String::from_utf8_lossy(&out.stdout)
    .lines()
    .any(|l| l.split_whitespace().next().is_some_and(|v| v.eq_ignore_ascii_case(name)))
}
