//! OpenTelemetry export: each Bot turn and tool call becomes a span sent as
//! OTLP/HTTP JSON to `<endpoint>/v1/traces`. Spans carry ids and names only
//! (Bot, chat, tool, outcome, duration) — never message text, arguments, or
//! results — so nothing sensitive leaves through telemetry. Off unless an
//! endpoint is set (in Settings, or forced by the admin policy).

use crate::runtime::Runtime;
use serde_json::{json, Value};

pub const SURFACE: &str = "asylum.agent";

/// The configured endpoint; the admin's wins.
pub fn endpoint(rt: &Runtime) -> Option<String> {
  let p = rt.policy().otel_endpoint;
  let e = if p.trim().is_empty() { rt.settings().otel_endpoint } else { p };
  let e = e.trim().trim_end_matches('/').to_string();
  (!e.is_empty()).then_some(e)
}

/// The OTLP request body for one span that ended now and lasted `ms`.
pub fn body(name: &str, attrs: &[(&str, &str)], ms: u64, now_ns: u128) -> Value {
  let id = uuid::Uuid::new_v4().simple().to_string();
  let start = now_ns.saturating_sub(u128::from(ms) * 1_000_000);
  let mut attributes: Vec<Value> = attrs.iter().map(|(k, v)| json!({ "key": format!("asylum.{k}"), "value": { "stringValue": v } })).collect();
  attributes.push(json!({ "key": "asylum.surface", "value": { "stringValue": SURFACE } }));
  json!({
    "resourceSpans": [{
      "resource": { "attributes": [{ "key": "service.name", "value": { "stringValue": "asylum" } }] },
      "scopeSpans": [{
        "scope": { "name": "asylum" },
        "spans": [{
          "traceId": id,
          "spanId": &id[..16],
          "name": name,
          "kind": 1,
          "startTimeUnixNano": start.to_string(),
          "endTimeUnixNano": now_ns.to_string(),
          "attributes": attributes,
        }]
      }]
    }]
  })
}

/// Send a span in the background; failures are dropped.
pub fn span(rt: &Runtime, name: &str, attrs: &[(&str, &str)], ms: u64) {
  let Some(base) = endpoint(rt) else { return };
  let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
  let payload = body(name, attrs, ms, now);
  tokio::spawn(async move {
    let _ = reqwest::Client::new().post(format!("{base}/v1/traces")).json(&payload).timeout(std::time::Duration::from_secs(5)).send().await;
  });
}

#[cfg(test)]
#[path = "../tests/otel.rs"]
mod tests;
