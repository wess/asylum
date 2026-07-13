//! Process-runtime host for ADE plugins.
//!
//! A plugin's `[runtime]` is an executable the app talks to over JSON on
//! stdin/stdout. Each exchange is a single newline-terminated JSON [`Request`]
//! answered by a single newline-terminated [`Response`]. This crate provides two
//! ways to run it:
//!
//! - [`invoke_once`] — spawn the command, send one request, read one response,
//!   let the process exit. Used for infrequent panel renders and tool calls.
//! - [`Session`] — keep a `persistent` runtime warm and call it repeatedly.
//!
//! The `wasm` runtime tier ([`plugin::RuntimeKind::Wasm`]) runs in-process under
//! `wasmi` — see [`wasm`] and [`invoke_wasm`]. It is sandboxed: the host links
//! only the capability functions the plugin declared.
//!
//! The protocol is transport-only: this crate never interprets a method name,
//! only frames the bytes. The app supplies methods and handles results.

pub mod wasm;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use wasm::WasmRuntime;

use plugin::{Runtime, RuntimeKind};

/// A runtime invocation failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not spawn runtime: {0}")]
    Spawn(String),
    #[error("runtime i/o: {0}")]
    Io(String),
    #[error("runtime closed its output before replying")]
    Closed,
    #[error("malformed runtime response: {0}")]
    Protocol(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("this runtime kind has no process to spawn (use invoke_wasm)")]
    Unsupported,
}

/// A request sent to a plugin runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// A response from a plugin runtime. Exactly one of `result` / `error` is set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    #[serde(default)]
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Split a runtime `command` string into (program, args) on ASCII whitespace.
fn split_command(command: &str) -> Option<(String, Vec<String>)> {
    let mut parts = command.split_whitespace().map(str::to_string);
    let program = parts.next()?;
    Some((program, parts.collect()))
}

/// Spawn a runtime process with piped stdio, in `cwd`. Rejects a `wasm` runtime.
pub fn spawn(runtime: &Runtime, cwd: &std::path::Path) -> Result<Child, Error> {
    if runtime.kind == RuntimeKind::Wasm {
        return Err(Error::Unsupported);
    }
    let (program, args) =
        split_command(&runtime.command).ok_or_else(|| Error::Spawn("empty command".into()))?;
    Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| Error::Spawn(e.to_string()))
}

/// Load a `wasm` runtime's module (relative to `plugin_dir`), instantiate it
/// with `capabilities`, and call `method` with `params`. This is the WASM-tier
/// analogue of [`invoke_once`].
pub fn invoke_wasm(
    runtime: &Runtime,
    plugin_dir: &Path,
    capabilities: &[String],
    method: &str,
    params: &Value,
) -> Result<Value, Error> {
    let rel = runtime
        .wasm
        .as_deref()
        .ok_or_else(|| Error::Spawn("wasm runtime has no module path".into()))?;
    let path = plugin_dir.join(rel);
    let bytes = std::fs::read(&path).map_err(|e| Error::Spawn(e.to_string()))?;
    let mut rt = WasmRuntime::new(&bytes, capabilities)?;
    rt.call(method, params)
}

/// One-shot invoke: spawn, send one request, read one response, and reap.
pub fn invoke_once(
    runtime: &Runtime,
    cwd: &std::path::Path,
    method: &str,
    params: Value,
) -> Result<Value, Error> {
    let mut child = spawn(runtime, cwd)?;
    let req = Request {
        id: 1,
        method: method.to_string(),
        params,
    };
    {
        let stdin = child.stdin.as_mut().ok_or(Error::Closed)?;
        write_request(stdin, &req)?;
    }
    // Dropping stdin signals EOF to one-shot runtimes.
    drop(child.stdin.take());

    let stdout = child.stdout.take().ok_or(Error::Closed)?;
    let mut reader = BufReader::new(stdout);
    let response = read_response(&mut reader)?;
    let _ = child.wait();
    unwrap_response(response)
}

/// A warm, persistent runtime: one long-lived process answering many calls.
pub struct Session {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    next_id: u64,
}

impl Session {
    /// Start a persistent runtime in `cwd`.
    pub fn start(runtime: &Runtime, cwd: &std::path::Path) -> Result<Self, Error> {
        let mut child = spawn(runtime, cwd)?;
        let stdin = child.stdin.take().ok_or(Error::Closed)?;
        let stdout = child.stdout.take().ok_or(Error::Closed)?;
        Ok(Self {
            child,
            stdin,
            reader: BufReader::new(stdout),
            next_id: 0,
        })
    }

    /// Send a request and block for its response.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, Error> {
        self.next_id += 1;
        let req = Request {
            id: self.next_id,
            method: method.to_string(),
            params,
        };
        write_request(&mut self.stdin, &req)?;
        let response = read_response(&mut self.reader)?;
        unwrap_response(response)
    }

    /// Terminate the runtime.
    pub fn shutdown(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn write_request(w: &mut impl Write, req: &Request) -> Result<(), Error> {
    let line = serde_json::to_string(req).map_err(|e| Error::Protocol(e.to_string()))?;
    w.write_all(line.as_bytes())
        .and_then(|_| w.write_all(b"\n"))
        .and_then(|_| w.flush())
        .map_err(|e| Error::Io(e.to_string()))
}

/// Read newline-delimited lines until one parses as a [`Response`]. Non-JSON
/// lines (a runtime's stray logging) are skipped.
fn read_response(reader: &mut impl BufRead) -> Result<Response, Error> {
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).map_err(|e| Error::Io(e.to_string()))?;
        if n == 0 {
            return Err(Error::Closed);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Response>(trimmed) {
            Ok(resp) => return Ok(resp),
            // Tolerate interleaved non-response chatter.
            Err(_) if !looks_like_json_object(trimmed) => continue,
            Err(e) => return Err(Error::Protocol(e.to_string())),
        }
    }
}

fn looks_like_json_object(s: &str) -> bool {
    s.starts_with('{')
}

fn unwrap_response(resp: Response) -> Result<Value, Error> {
    if let Some(err) = resp.error {
        return Err(Error::Runtime(err));
    }
    Ok(resp.result.unwrap_or(Value::Null))
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
