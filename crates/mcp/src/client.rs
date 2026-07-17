//! Talking to an upstream MCP server - the half where the gateway is a *client*.
//!
//! An [`Upstream`] is the seam the [`host`](crate::host) drives: send a request,
//! get the JSON-RPC reply; fire a notification. Two transports implement it:
//!
//! - [`StdioUpstream`] launches a local server and speaks newline-framed JSON
//!   over its stdio (the common case: `github`, filesystem, etc.). The framing
//!   is factored into [`StdioConn`], generic over any reader/writer, so it is
//!   unit-tested against in-memory buffers with no process spawned.
//! - [`HttpUpstream`] POSTs to a remote MCP endpoint via `curl` (for TLS), with
//!   any auth secret kept off `argv` by passing it through curl's `--config`
//!   stdin, exactly as the secrets proxy does.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;

use serde_json::Value;

use crate::jsonrpc::{self, Response};

/// One upstream MCP server, as a request/notify seam. Implementations serialize
/// their own access (a single outstanding request at a time), so the trait takes
/// `&self`.
pub trait Upstream: Send + Sync {
    /// Send request `method` with `params`; return the correlated JSON-RPC reply
    /// (which may itself be a JSON-RPC error [`Response`]).
    fn request(&self, method: &str, params: Value) -> Result<Response, String>;
    /// Fire a notification (no reply owed).
    fn notify(&self, method: &str, params: Value) -> Result<(), String>;
}

/// Safety valve: at most one request is outstanding on a connection, so any
/// reply carrying an id other than ours is an interleaved server notification or
/// log line to skip. Bound how many we skip before giving up, so a misbehaving
/// server can never wedge a worker in an unbounded read loop.
const MAX_INTERLEAVED: usize = 256;

// --- stdio ------------------------------------------------------------------

/// Newline-framed JSON-RPC over a reader/writer pair. Generic so the framing is
/// tested against in-memory buffers; [`StdioUpstream`] instantiates it over a
/// child process's pipes.
pub struct StdioConn<R, W> {
    reader: R,
    writer: W,
    next_id: i64,
}

impl<R: BufRead, W: Write> StdioConn<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            next_id: 0,
        }
    }

    fn write_message(&mut self, value: &Value) -> Result<(), String> {
        let mut line = value.to_string();
        line.push('\n');
        self.writer
            .write_all(line.as_bytes())
            .and_then(|_| self.writer.flush())
            .map_err(|e| format!("write to upstream failed: {e}"))
    }

    /// Send a request and read frames until the reply with our id arrives.
    pub fn request(&mut self, method: &str, params: Value) -> Result<Response, String> {
        self.next_id += 1;
        let id = self.next_id;
        self.write_message(&jsonrpc::request_envelope(id, method, params))?;

        for _ in 0..MAX_INTERLEAVED {
            let mut line = String::new();
            let n = self
                .reader
                .read_line(&mut line)
                .map_err(|e| format!("read from upstream failed: {e}"))?;
            if n == 0 {
                return Err("upstream closed the connection".into());
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            // Only a reply carrying our id is ours; skip the server's own
            // notifications and any log chatter.
            if value.get("id") == Some(&Value::from(id)) {
                return jsonrpc::parse_reply(&value)
                    .ok_or_else(|| "upstream reply had neither result nor error".to_string());
            }
        }
        Err("no matching reply from upstream".into())
    }

    pub fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.write_message(&jsonrpc::notification_envelope(method, params))
    }
}

type ChildConn = StdioConn<BufReader<ChildStdout>, ChildStdin>;

struct StdioInner {
    child: Child,
    conn: ChildConn,
}

/// A local MCP server launched as a child process, spoken to over its stdio.
pub struct StdioUpstream {
    inner: Mutex<StdioInner>,
}

impl StdioUpstream {
    /// Spawn `command` with `args` and `env` (added on top of the inherited
    /// environment) and wrap its stdio. stderr is inherited so a failing server's
    /// diagnostics reach the app's log rather than being swallowed.
    pub fn spawn(
        command: &str,
        args: &[String],
        env: &[(String, String)],
    ) -> Result<StdioUpstream, String> {
        let mut child = Command::new(command)
            .args(args)
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("could not launch `{command}`: {e}"))?;
        let stdout = child.stdout.take().ok_or("child produced no stdout pipe")?;
        let stdin = child.stdin.take().ok_or("child produced no stdin pipe")?;
        let conn = StdioConn::new(BufReader::new(stdout), stdin);
        Ok(StdioUpstream {
            inner: Mutex::new(StdioInner { child, conn }),
        })
    }
}

impl Upstream for StdioUpstream {
    fn request(&self, method: &str, params: Value) -> Result<Response, String> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.conn.request(method, params)
    }
    fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.conn.notify(method, params)
    }
}

impl Drop for StdioUpstream {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.inner.lock() {
            let _ = inner.child.kill();
            let _ = inner.child.wait();
        }
    }
}

// --- http -------------------------------------------------------------------

/// A remote MCP server reached over Streamable HTTP (`curl` for TLS). Stateless
/// on our side except for the negotiated `Mcp-Session-Id`, which we echo back on
/// later requests once the server hands us one.
pub struct HttpUpstream {
    url: String,
    /// `(header_name, header_value)` for auth; the value carries the secret and
    /// is passed to curl off-argv. `None` when the upstream needs no auth.
    auth: Option<(String, String)>,
    session: Mutex<Option<String>>,
    next_id: AtomicI64,
}

impl HttpUpstream {
    pub fn new(url: &str, auth: Option<(String, String)>) -> HttpUpstream {
        HttpUpstream {
            url: url.to_string(),
            auth,
            session: Mutex::new(None),
            next_id: AtomicI64::new(0),
        }
    }

    fn post(&self, envelope: &Value, want_reply: bool) -> Result<Option<Response>, String> {
        let session = self
            .session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let outcome = http_post(&self.url, envelope, self.auth.as_ref(), session.as_deref())?;
        if let Some(id) = outcome.session_id {
            *self.session.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
        }
        if !want_reply {
            return Ok(None);
        }
        let value: Value = serde_json::from_str(&outcome.body)
            .map_err(|e| format!("upstream returned non-JSON body: {e}"))?;
        jsonrpc::parse_reply(&value)
            .map(Some)
            .ok_or_else(|| "upstream reply had neither result nor error".to_string())
    }
}

impl Upstream for HttpUpstream {
    fn request(&self, method: &str, params: Value) -> Result<Response, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let envelope = jsonrpc::request_envelope(id, method, params);
        self.post(&envelope, true)?
            .ok_or_else(|| "no reply from upstream".to_string())
    }
    fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        let envelope = jsonrpc::notification_envelope(method, params);
        self.post(&envelope, false).map(|_| ())
    }
}

/// The result of one HTTP POST to an MCP endpoint.
struct HttpOutcome {
    session_id: Option<String>,
    body: String,
}

/// POST `envelope` to `url` via curl. The optional `auth` header (secret-bearing)
/// is written to curl's `--config` stdin so it never lands on argv; the request
/// body goes the same way. Response headers and body come back via `-i`, and the
/// body is unwrapped from an SSE frame if the server chose to stream.
fn http_post(
    url: &str,
    envelope: &Value,
    auth: Option<&(String, String)>,
    session: Option<&str>,
) -> Result<HttpOutcome, String> {
    let mut args: Vec<String> = vec![
        "-sS".into(),
        "-i".into(),
        "--connect-timeout".into(),
        "15".into(),
        "--max-time".into(),
        "120".into(),
        "-X".into(),
        "POST".into(),
        url.to_string(),
        "-H".into(),
        "Content-Type: application/json".into(),
        "-H".into(),
        "Accept: application/json, text/event-stream".into(),
    ];
    if let Some(id) = session {
        args.push("-H".into());
        args.push(format!("Mcp-Session-Id: {id}"));
    }
    // Auth header + body via --config stdin, keeping the secret off argv.
    args.push("--config".into());
    args.push("-".into());

    let mut config = String::new();
    if let Some((name, value)) = auth {
        config.push_str(&format!(
            "header = \"{}: {}\"\n",
            curl_escape(name),
            curl_escape(value)
        ));
    }
    config.push_str(&format!(
        "data = \"{}\"\n",
        curl_escape(&envelope.to_string())
    ));

    let mut child = Command::new("curl")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run curl: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(config.as_bytes());
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("curl failed: {e}"))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "upstream request failed: {}",
            why.trim().split('\n').next_back().unwrap_or("curl error")
        ));
    }
    Ok(parse_http_response(&String::from_utf8_lossy(&out.stdout)))
}

/// Split a `curl -i` response into its session id (if any) and its JSON body,
/// unwrapping a `text/event-stream` frame down to the `data:` payload.
///
/// The body follows the *first* blank-line separator, not the last: an SSE body
/// ends with its own blank line, so splitting from the end would truncate it.
/// Interim `1xx` header blocks (a `100 Continue` when curl streams the request
/// body) are skipped so the real response is the one parsed.
fn parse_http_response(raw: &str) -> HttpOutcome {
    let mut rest = raw;
    loop {
        let Some((head, body)) = split_head(rest) else {
            return HttpOutcome {
                session_id: None,
                body: rest.trim().to_string(),
            };
        };
        // Skip an interim 1xx block, or a leftover header block if curl emitted
        // more than one before the final response.
        if is_interim(head) || body.trim_start().starts_with("HTTP/") {
            rest = body;
            continue;
        }
        let session_id = head
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("mcp-session-id:"))
            .and_then(|l| l.split_once(':'))
            .map(|(_, v)| v.trim().to_string());
        let is_sse = head.lines().any(|l| {
            l.to_ascii_lowercase().starts_with("content-type:") && l.contains("event-stream")
        });
        let body = if is_sse {
            sse_data(body)
        } else {
            body.trim().to_string()
        };
        return HttpOutcome { session_id, body };
    }
}

/// Split off the first header block (up to the first blank line) from the rest.
fn split_head(raw: &str) -> Option<(&str, &str)> {
    raw.split_once("\r\n\r\n")
        .or_else(|| raw.split_once("\n\n"))
}

/// Whether a header block's status line is an interim `1xx` response.
fn is_interim(head: &str) -> bool {
    head.lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .is_some_and(|code| (100..200).contains(&code))
}

/// Concatenate the `data:` lines of an SSE frame into the JSON payload.
fn sse_data(body: &str) -> String {
    body.lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("")
}

/// Escape a value for a curl `--config` double-quoted string (same rules as the
/// proxy's forwarder: quotes, backslashes, and the newlines that would otherwise
/// end the directive).
fn curl_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

#[cfg(test)]
#[path = "../tests/client.rs"]
mod tests;
