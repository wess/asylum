//! `claude -p --output-format stream-json` lines: text deltas as they
//! arrive, and a final result with usage (or an error).

use anyhow::{bail, Result};
use chat::{StreamEvent, Usage};
use serde_json::Value;

#[derive(Default)]
pub struct Lines {
  text: String,
  result: Option<String>,
  usage: Option<Usage>,
  failure: Option<String>,
  streamed: bool,
}

impl Lines {
  pub fn push(&mut self, line: &str) -> Vec<StreamEvent> {
    let Ok(v) = serde_json::from_str::<Value>(line.trim()) else { return Vec::new() };
    match v["type"].as_str() {
      Some("stream_event") => {
        let e = &v["event"];
        if e["type"] == "content_block_delta" && e["delta"]["type"] == "text_delta" {
          if let Some(t) = e["delta"]["text"].as_str() {
            self.text.push_str(t);
            self.streamed = true;
            return vec![StreamEvent::Text(t.to_string())];
          }
        }
        if e["type"] == "content_block_delta" && e["delta"]["type"] == "thinking_delta" {
          if let Some(t) = e["delta"]["thinking"].as_str() {
            return vec![StreamEvent::Reasoning(t.to_string())];
          }
        }
        Vec::new()
      }
      Some("result") => {
        self.result = v["result"].as_str().map(str::to_string);
        if v["is_error"].as_bool() == Some(true) {
          self.failure = Some(self.result.clone().filter(|r| !r.trim().is_empty()).unwrap_or_else(|| v["subtype"].as_str().unwrap_or("error").to_string()));
        }
        let u = &v["usage"];
        let n = |k: &str| u[k].as_u64().unwrap_or(0);
        let prompt = n("input_tokens") + n("cache_read_input_tokens") + n("cache_creation_input_tokens");
        let completion = n("output_tokens");
        self.usage = Some(Usage { prompt_tokens: prompt, completion_tokens: completion, total_tokens: prompt + completion });
        Vec::new()
      }
      _ => Vec::new(),
    }
  }

  pub fn streamed(&self) -> bool {
    self.streamed
  }

  pub fn finish(&mut self) -> Result<(String, Option<Usage>)> {
    if let Some(f) = self.failure.take() {
      bail!("the provider reported an error: {f}");
    }
    let text = if self.text.is_empty() { self.result.take().unwrap_or_default() } else { std::mem::take(&mut self.text) };
    Ok((text, self.usage))
  }
}

#[cfg(test)]
#[path = "../tests/streamjson.rs"]
mod tests;
