use serde_json::{json, Value};

pub const PROTOCOL: &str = "2025-06-18";

pub fn request(id: u64, method: &str, params: Value) -> Value {
  json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

pub fn notification(method: &str, params: Value) -> Value {
  json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

/// The `result` of a response, or its error as text.
pub fn result(v: Value) -> anyhow::Result<Value> {
  if let Some(err) = v.get("error") {
    let msg = err["message"].as_str().unwrap_or("MCP error");
    anyhow::bail!("{msg}");
  }
  Ok(v.get("result").cloned().unwrap_or(Value::Null))
}
