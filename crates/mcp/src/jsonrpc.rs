//! Minimal JSON-RPC 2.0 for MCP - the wire format every MCP message rides on,
//! over both transports the gateway speaks (newline-framed stdio to upstreams,
//! HTTP POST from agents). Pure: parse a message, build a response. The 2025
//! MCP revisions carry one message per frame (batching was removed), so this
//! deliberately handles a single object, not a top-level array.

use serde_json::{json, Value};

/// Standard JSON-RPC error codes (plus MCP's convention of reusing them).
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

/// One inbound JSON-RPC message. A message with no `id` is a *notification*
/// (fire-and-forget, no response is owed); otherwise it is a request whose `id`
/// must be echoed on the response.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// The correlation id, or `None` for a notification. Kept as a raw [`Value`]
    /// because JSON-RPC permits a number, string, or null.
    pub id: Option<Value>,
    pub method: String,
    /// The `params` object, or [`Value::Null`] when omitted.
    pub params: Value,
}

impl Request {
    /// Parse one JSON-RPC message. Returns `Err(response)` already shaped as the
    /// JSON-RPC error the caller should send back (parse error / invalid
    /// request), so a malformed frame never needs ad-hoc handling upstream. The
    /// error is boxed: a `Response` carries JSON values and would otherwise make
    /// the common `Ok` path pay for the rare error's size.
    pub fn parse(raw: &str) -> Result<Request, Box<Response>> {
        let value: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => return Err(Box::new(Response::error(Value::Null, PARSE_ERROR, "parse error"))),
        };
        Request::from_value(value)
    }

    /// Interpret an already-parsed JSON value as a request.
    pub fn from_value(value: Value) -> Result<Request, Box<Response>> {
        let obj = value.as_object().ok_or_else(|| {
            Box::new(Response::error(Value::Null, INVALID_REQUEST, "not a JSON object"))
        })?;
        // `id` may legitimately be null or absent; we treat an absent id as a
        // notification. A present-but-null id is also a notification per
        // JSON-RPC (no response correlatable), which is how notifications look.
        let id = match obj.get("id") {
            None | Some(Value::Null) => None,
            Some(other) => Some(other.clone()),
        };
        let method = match obj.get("method").and_then(Value::as_str) {
            Some(m) => m.to_string(),
            None => {
                return Err(Box::new(Response::error(
                    id.clone().unwrap_or(Value::Null),
                    INVALID_REQUEST,
                    "missing method",
                )))
            }
        };
        let params = obj.get("params").cloned().unwrap_or(Value::Null);
        Ok(Request { id, method, params })
    }

    /// Whether this is a notification (no response owed).
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }

    /// A `params` sub-field as a string, if present.
    pub fn param_str(&self, key: &str) -> Option<&str> {
        self.params.get(key).and_then(Value::as_str)
    }
}

/// An outbound JSON-RPC response. Held as the finished `result`/`error` value so
/// the transport just has to serialize it; `id` is echoed from the request.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub id: Value,
    pub payload: Payload,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Payload {
    Result(Value),
    Error { code: i64, message: String },
}

impl Response {
    /// A success response carrying `result`.
    pub fn result(id: Value, result: Value) -> Response {
        Response {
            id,
            payload: Payload::Result(result),
        }
    }

    /// An error response.
    pub fn error(id: Value, code: i64, message: impl Into<String>) -> Response {
        Response {
            id,
            payload: Payload::Error {
                code,
                message: message.into(),
            },
        }
    }

    /// Serialize to the JSON-RPC envelope.
    pub fn to_value(&self) -> Value {
        match &self.payload {
            Payload::Result(result) => json!({
                "jsonrpc": "2.0",
                "id": self.id,
                "result": result,
            }),
            Payload::Error { code, message } => json!({
                "jsonrpc": "2.0",
                "id": self.id,
                "error": { "code": code, "message": message },
            }),
        }
    }

    pub fn to_json_string(&self) -> String {
        self.to_value().to_string()
    }

    /// Whether this response is an error (used when relaying an upstream reply).
    pub fn is_error(&self) -> bool {
        matches!(self.payload, Payload::Error { .. })
    }
}

/// Build an outbound request envelope for `method`/`params` with correlation
/// `id`. Used by the upstream clients when the gateway acts as an MCP *client*.
pub fn request_envelope(id: i64, method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

/// Build a notification envelope (no id) for `method`/`params`.
pub fn notification_envelope(method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    })
}

/// Parse an upstream's JSON-RPC *reply* envelope into a [`Response`], matching on
/// the presence of `result` vs `error`. `id` is whatever the upstream echoed.
pub fn parse_reply(value: &Value) -> Option<Response> {
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    if let Some(result) = value.get("result") {
        return Some(Response::result(id, result.clone()));
    }
    if let Some(error) = value.get("error") {
        let code = error.get("code").and_then(Value::as_i64).unwrap_or(INTERNAL_ERROR);
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("upstream error")
            .to_string();
        return Some(Response::error(id, code, message));
    }
    None
}

#[cfg(test)]
#[path = "../tests/jsonrpc.rs"]
mod tests;
