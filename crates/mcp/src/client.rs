use crate::http::Http;
use crate::rpc;
use crate::stdio::Stdio;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tool {
  pub name: String,
  #[serde(default)]
  pub description: String,
  #[serde(rename = "inputSchema", default = "empty_schema")]
  pub input_schema: Value,
  #[serde(default)]
  pub annotations: Value,
}

impl Tool {
  /// Whether the server says this tool only reads.
  pub fn read_only(&self) -> bool {
    self.annotations["readOnlyHint"].as_bool().unwrap_or(false)
  }
}

fn empty_schema() -> Value {
  json!({ "type": "object", "properties": {} })
}

enum Transport {
  Stdio(Stdio),
  Http(Http),
}

pub struct Client {
  transport: Transport,
  next: AtomicU64,
}

impl Client {
  pub fn stdio(s: Stdio) -> Self {
    Self { transport: Transport::Stdio(s), next: AtomicU64::new(1) }
  }

  pub fn http(h: Http) -> Self {
    Self { transport: Transport::Http(h), next: AtomicU64::new(1) }
  }

  async fn call(&self, method: &str, params: Value) -> Result<Value> {
    let id = self.next.fetch_add(1, Ordering::Relaxed);
    let msg = rpc::request(id, method, params);
    let reply = match &self.transport {
      Transport::Stdio(s) => s.call(&msg).await?,
      Transport::Http(h) => h.post(&msg).await?.unwrap_or(Value::Null),
    };
    rpc::result(reply)
  }

  async fn notify(&self, method: &str) -> Result<()> {
    let msg = rpc::notification(method, json!({}));
    match &self.transport {
      Transport::Stdio(s) => s.send(&msg).await,
      Transport::Http(h) => h.post(&msg).await.map(|_| ()),
    }
  }

  pub async fn initialize(&self) -> Result<Value> {
    let info = self
      .call(
        "initialize",
        json!({
          "protocolVersion": rpc::PROTOCOL,
          "capabilities": {},
          "clientInfo": { "name": "asylum", "version": env!("CARGO_PKG_VERSION") }
        }),
      )
      .await?;
    self.notify("notifications/initialized").await?;
    Ok(info)
  }

  pub async fn tools(&self) -> Result<Vec<Tool>> {
    let mut out = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
      let params = match &cursor {
        Some(c) => json!({ "cursor": c }),
        None => json!({}),
      };
      let res = self.call("tools/list", params).await?;
      let page: Vec<Tool> = serde_json::from_value(res["tools"].clone()).unwrap_or_default();
      out.extend(page);
      match res["nextCursor"].as_str() {
        Some(c) if !c.is_empty() => cursor = Some(c.to_string()),
        _ => return Ok(out),
      }
    }
  }

  /// Call a tool; the result's content flattened to text.
  pub async fn call_tool(&self, name: &str, args: Value) -> Result<(String, bool)> {
    let res = self.call("tools/call", json!({ "name": name, "arguments": args })).await?;
    Ok((flatten(&res), res["isError"].as_bool().unwrap_or(false)))
  }
}

pub fn flatten(res: &Value) -> String {
  let mut parts = Vec::new();
  for c in res["content"].as_array().into_iter().flatten() {
    match c["type"].as_str() {
      Some("text") => parts.push(c["text"].as_str().unwrap_or("").to_string()),
      Some("image") => parts.push("[image]".into()),
      Some("resource") => parts.push(c["resource"]["text"].as_str().unwrap_or("[resource]").to_string()),
      Some("resource_link") => parts.push(format!("[{}]", c["uri"].as_str().unwrap_or("resource"))),
      _ => {}
    }
  }
  if parts.is_empty() {
    if let Some(s) = res.get("structuredContent") {
      return s.to_string();
    }
  }
  parts.join("\n")
}

#[cfg(test)]
#[path = "../tests/client.rs"]
mod tests;
