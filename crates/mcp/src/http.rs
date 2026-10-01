//! Streamable HTTP: each JSON-RPC message is a POST; the reply is JSON or an
//! SSE stream carrying it. The server may assign a session id.

use anyhow::{bail, Result};
use futures::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::Mutex;

pub struct Http {
  client: reqwest::Client,
  url: String,
  headers: HashMap<String, String>,
  token: Mutex<Option<String>>,
  session: Mutex<Option<String>>,
}

/// A 401 from the server: the caller should (re)authorize.
#[derive(Debug)]
pub struct Unauthorized {
  pub resource_metadata: Option<String>,
}

impl std::fmt::Display for Unauthorized {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "the server needs authorization")
  }
}

impl std::error::Error for Unauthorized {}

impl Http {
  pub fn new(url: &str, headers: HashMap<String, String>, token: Option<String>) -> Self {
    Self {
      client: reqwest::Client::new(),
      url: url.to_string(),
      headers,
      token: Mutex::new(token),
      session: Mutex::new(None),
    }
  }

  pub async fn set_token(&self, token: Option<String>) {
    *self.token.lock().await = token;
  }

  pub async fn post(&self, msg: &Value) -> Result<Option<Value>> {
    let mut req = self
      .client
      .post(&self.url)
      .header("Accept", "application/json, text/event-stream")
      .header("MCP-Protocol-Version", crate::rpc::PROTOCOL)
      .json(msg);
    for (k, v) in &self.headers {
      req = req.header(k, v);
    }
    if let Some(t) = self.token.lock().await.as_ref() {
      req = req.bearer_auth(t);
    }
    if let Some(s) = self.session.lock().await.as_ref() {
      req = req.header("Mcp-Session-Id", s);
    }
    let res = req.send().await?;
    if res.status() == reqwest::StatusCode::UNAUTHORIZED {
      let meta = res
        .headers()
        .get("www-authenticate")
        .and_then(|v| v.to_str().ok())
        .and_then(resource_metadata);
      return Err(Unauthorized { resource_metadata: meta }.into());
    }
    if !res.status().is_success() {
      let status = res.status();
      let body = res.text().await.unwrap_or_default();
      bail!("{status}: {}", body.chars().take(300).collect::<String>());
    }
    if let Some(s) = res.headers().get("mcp-session-id").and_then(|v| v.to_str().ok()) {
      *self.session.lock().await = Some(s.to_string());
    }
    let is_sse = res
      .headers()
      .get("content-type")
      .and_then(|v| v.to_str().ok())
      .is_some_and(|c| c.contains("event-stream"));
    if msg.get("id").is_none() {
      return Ok(None);
    }
    if !is_sse {
      let text = res.text().await?;
      if text.trim().is_empty() {
        return Ok(None);
      }
      return Ok(Some(serde_json::from_str(&text)?));
    }
    let id = msg["id"].clone();
    let mut stream = res.bytes_stream();
    let mut buf = String::new();
    while let Some(chunk) = stream.next().await {
      buf.push_str(&String::from_utf8_lossy(&chunk?));
      while let Some(pos) = buf.find('\n') {
        let line: String = buf.drain(..=pos).collect();
        let Some(data) = line.trim_end().strip_prefix("data:") else { continue };
        let Ok(v) = serde_json::from_str::<Value>(data.trim()) else { continue };
        if v.get("id") == Some(&id) {
          return Ok(Some(v));
        }
      }
    }
    bail!("the server closed the stream without replying")
  }
}

/// `resource_metadata="..."` from a WWW-Authenticate header.
pub fn resource_metadata(header: &str) -> Option<String> {
  let i = header.find("resource_metadata=")? + "resource_metadata=".len();
  let rest = header[i..].trim_start_matches('"');
  let end = rest.find(['"', ',']).unwrap_or(rest.len());
  Some(rest[..end].to_string())
}

#[cfg(test)]
#[path = "../tests/http.rs"]
mod tests;
