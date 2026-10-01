use crate::accumulate::Accumulator;
use crate::sse::{Decoder, Frame};
use crate::types::{Chunk, Model, Request, StreamEvent};
use anyhow::{anyhow, bail, Result};
use futures::{Stream, StreamExt};
use serde::Deserialize;

pub const DEFAULT_BASE: &str = "https://api.x.ai/v1";
pub const DEFAULT_MODEL: &str = "grok-4";

/// An OpenAI-compatible chat-completions endpoint: xAI, OpenAI, Ollama,
/// LiteLLM, or any gateway. Transient failures (connection errors, 408,
/// 429, 5xx) are retried with jittered exponential backoff before any text
/// arrives; other client errors fail at once.
#[derive(Clone)]
pub struct Client {
  http: reqwest::Client,
  base: String,
  key: String,
  retries: usize,
}

#[derive(Deserialize)]
struct ModelList {
  data: Vec<Model>,
}

impl Client {
  pub fn new(key: impl Into<String>, base: Option<String>) -> Self {
    // No read timeout: a local model can think for minutes before its first token.
    let http = reqwest::Client::builder()
      .connect_timeout(std::time::Duration::from_secs(15))
      .build()
      .unwrap_or_default();
    Self {
      http,
      retries: 3,
      base: base
        .filter(|b| !b.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_BASE.into())
        .trim_end_matches('/')
        .to_string(),
      key: key.into(),
    }
  }

  pub fn retries(mut self, n: usize) -> Self {
    self.retries = n;
    self
  }

  pub fn base(&self) -> &str {
    &self.base
  }

  fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    if self.key.is_empty() {
      req
    } else {
      req.bearer_auth(&self.key)
    }
  }

  /// Send, retrying transient failures.
  async fn send(&self, build: impl Fn() -> reqwest::RequestBuilder) -> Result<reqwest::Response> {
    let mut attempt = 0usize;
    loop {
      attempt += 1;
      match self.auth(build()).send().await {
        Ok(res) if res.status().is_success() => return Ok(res),
        Ok(res) if retryable(res.status()) && attempt <= self.retries => {
          let wait = retry_after(&res).unwrap_or_else(|| backoff(attempt));
          tokio_sleep(wait).await;
        }
        Ok(res) => return check(res).await,
        Err(e) if attempt <= self.retries && (e.is_connect() || e.is_timeout()) => tokio_sleep(backoff(attempt)).await,
        Err(e) => return Err(e.into()),
      }
    }
  }

  pub async fn models(&self) -> Result<Vec<Model>> {
    let res = self.send(|| self.http.get(format!("{}/models", self.base))).await?;
    Ok(res.json::<ModelList>().await?.data)
  }

  /// One non-streaming completion, returning the text. Used for background
  /// jobs (titles, memory extraction, skill drafting) where no one watches.
  pub async fn complete(&self, mut req: Request) -> Result<String> {
    req.stream = false;
    req.stream_options = None;
    let res = self.send(|| self.http.post(format!("{}/chat/completions", self.base)).json(&req)).await?;
    let body: serde_json::Value = res.json().await?;
    body["choices"][0]["message"]["content"]
      .as_str()
      .map(str::to_string)
      .ok_or_else(|| anyhow!("empty completion"))
  }

  pub async fn stream(&self, req: Request) -> Result<impl Stream<Item = Result<StreamEvent>>> {
    let res = self.send(|| self.http.post(format!("{}/chat/completions", self.base)).json(&req)).await?;
    Ok(decode(res.bytes_stream()))
  }
}

fn retryable(s: reqwest::StatusCode) -> bool {
  s == reqwest::StatusCode::REQUEST_TIMEOUT || s == reqwest::StatusCode::TOO_MANY_REQUESTS || s.is_server_error()
}

fn retry_after(res: &reqwest::Response) -> Option<std::time::Duration> {
  let secs: u64 = res.headers().get(reqwest::header::RETRY_AFTER)?.to_str().ok()?.trim().parse().ok()?;
  Some(std::time::Duration::from_secs(secs.min(60)))
}

/// 500ms, 1s, 2s, 4s, capped at 8s, jittered by about a quarter.
pub fn backoff(attempt: usize) -> std::time::Duration {
  use std::hash::{BuildHasher, Hasher};
  let base = 500u64 << (attempt.saturating_sub(1).min(4) as u32);
  let wobble = base / 4;
  let sample = std::collections::hash_map::RandomState::new().build_hasher().finish() % (wobble * 2 + 1);
  std::time::Duration::from_millis(base - wobble + sample)
}

async fn tokio_sleep(d: std::time::Duration) {
  tokio::time::sleep(d).await;
}

async fn check(res: reqwest::Response) -> Result<reqwest::Response> {
  if res.status().is_success() {
    return Ok(res);
  }
  let status = res.status();
  let body = res.text().await.unwrap_or_default();
  let msg = serde_json::from_str::<serde_json::Value>(&body)
    .ok()
    .and_then(|v| {
      v["error"]["message"]
        .as_str()
        .or(v["error"].as_str())
        .map(str::to_string)
    })
    .unwrap_or(body);
  bail!("{status}: {msg}")
}

struct State<S> {
  bytes: S,
  decoder: Decoder,
  calls: Accumulator,
  finish: Option<String>,
  queue: std::collections::VecDeque<StreamEvent>,
  done: bool,
}

fn decode<S, B>(bytes: S) -> impl Stream<Item = Result<StreamEvent>>
where
  S: Stream<Item = reqwest::Result<B>> + Unpin,
  B: AsRef<[u8]>,
{
  let state = State {
    bytes,
    decoder: Decoder::default(),
    calls: Accumulator::default(),
    finish: None,
    queue: Default::default(),
    done: false,
  };
  futures::stream::unfold(state, |mut st| async move {
    loop {
      if let Some(ev) = st.queue.pop_front() {
        return Some((Ok(ev), st));
      }
      if st.done {
        return None;
      }
      match st.bytes.next().await {
        Some(Ok(chunk)) => {
          for frame in st.decoder.push(chunk.as_ref()) {
            match frame {
              Frame::Done => st.done = true,
              Frame::Data(json) => apply(&mut st, &json),
            }
          }
          if st.done {
            finish(&mut st);
          }
        }
        Some(Err(e)) => {
          st.done = true;
          return Some((Err(e.into()), st));
        }
        None => {
          st.done = true;
          finish(&mut st);
        }
      }
    }
  })
}

fn apply<S>(st: &mut State<S>, json: &str) {
  let Ok(chunk) = serde_json::from_str::<Chunk>(json) else {
    return;
  };
  if let Some(u) = chunk.usage {
    st.queue.push_back(StreamEvent::Usage(u));
  }
  for choice in chunk.choices {
    if let Some(r) = choice.delta.reasoning_content.filter(|s| !s.is_empty()) {
      st.queue.push_back(StreamEvent::Reasoning(r));
    }
    if let Some(t) = choice.delta.content.filter(|s| !s.is_empty()) {
      st.queue.push_back(StreamEvent::Text(t));
    }
    for d in choice.delta.tool_calls.iter().flatten() {
      st.calls.push(d);
    }
    if choice.finish_reason.is_some() {
      st.finish = choice.finish_reason;
    }
  }
}

fn finish<S>(st: &mut State<S>) {
  let calls = std::mem::take(&mut st.calls).finish();
  st.queue.push_back(StreamEvent::Done {
    calls,
    finish: st.finish.take(),
  });
}

#[cfg(test)]
#[path = "../tests/client.rs"]
mod tests;
