//! `asylum mcp` - the aggregated MCP gateway from the shell or an agent.
//!
//! ```text
//! asylum mcp list          # services + tools the gateway currently exposes
//! asylum mcp serve [--bind addr]   # run a standalone gateway over configured servers
//! asylum mcp stdio         # bridge a stdio-only MCP client to the gateway over HTTP
//! asylum mcp skill         # print the agent skill doc
//! ```
//!
//! `list` and `stdio` talk to the running gateway via the env the app injects
//! ([`mcp::ENV_URL`] / [`mcp::ENV_TOKEN`]); `serve` stands one up from the user's
//! `settings.json` for a locally launched agent.

use std::io::{BufRead, Read, Write};
use std::net::TcpStream;

use serde_json::{json, Value};

pub fn mcp(args: &[String]) -> Result<(), String> {
    let sub = args.first().map(String::as_str).unwrap_or("");
    let rest = &args[1.min(args.len())..];
    match sub {
        "skill" => {
            println!("{}", mcp::SKILL);
            Ok(())
        }
        "list" => list(),
        "serve" => serve(rest),
        "stdio" => stdio(),
        _ => Err("usage: asylum mcp <list|serve|stdio|skill>".into()),
    }
}

/// `asylum mcp list` - initialize against the running gateway and print the
/// namespaced tools it exposes to this run.
fn list() -> Result<(), String> {
    let (base, token) = endpoint()?;
    // The gateway is stateless, but a real client initializes first; do the same.
    rpc(
        &base,
        &token,
        "initialize",
        json!({
            "protocolVersion": mcp::PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "asylum-cli", "version": env!("CARGO_PKG_VERSION") },
        }),
    )?;
    let result = rpc(&base, &token, "tools/list", json!({}))?;
    let tools = result
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if tools.is_empty() {
        println!("(no tools exposed)");
        return Ok(());
    }
    for tool in &tools {
        let name = tool.get("name").and_then(Value::as_str).unwrap_or("?");
        let desc = tool.get("description").and_then(Value::as_str).unwrap_or("");
        let desc = desc.lines().next().unwrap_or("");
        println!("{name}\t{desc}");
    }
    Ok(())
}

/// `asylum mcp serve` - run a standalone gateway over the configured servers, for
/// an agent launched outside the app. Secrets are resolved from the keep when a
/// passphrase is present, else omitted.
fn serve(args: &[String]) -> Result<(), String> {
    let loaded = config::load(&config::default_path());
    let settings = loaded.settings;
    let bind = crate::flag(args, "--bind")
        .map(String::from)
        .unwrap_or_else(|| settings.mcp.bind.clone());

    let keep = open_keep();
    let (host, warnings) = mcp::connect(&settings.mcp_servers, |project, name| {
        keep.as_ref().and_then(|k| {
            let scope = (project != 0).then_some(project);
            k.resolve(scope, name).map(str::to_string)
        })
    });
    for warning in &warnings {
        eprintln!("warning: {warning}");
    }
    if host.is_empty() {
        eprintln!("no mcp servers connected (configure `mcp_servers` in settings.json)");
    }

    let expose = mcp::Expose::parse(&settings.mcp.expose);
    let gateway = mcp::Gateway::new(host, expose);
    eprintln!("asylum mcp gateway → {}", mcp::describe(&bind));
    mcp::serve(bind.as_str(), gateway).map_err(|e| e.to_string())
}

/// `asylum mcp stdio` - a bridge for a CLI that speaks MCP only over stdio: read
/// newline-framed JSON-RPC from stdin, forward each message to the gateway over
/// HTTP, and write replies back to stdout.
fn stdio() -> Result<(), String> {
    let (base, token) = endpoint()?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (status, body) = post(&base, &token, line)?;
        // A notification is answered 202 with no body; nothing to write back.
        if status == 202 || body.trim().is_empty() {
            continue;
        }
        writeln!(stdout, "{body}").map_err(|e| e.to_string())?;
        stdout.flush().ok();
    }
    Ok(())
}

/// The gateway endpoint and token from the injected environment.
fn endpoint() -> Result<(String, String), String> {
    let base = std::env::var(mcp::ENV_URL).map_err(|_| {
        format!(
            "{} unset (not inside an Asylum run, or the gateway is disabled)",
            mcp::ENV_URL
        )
    })?;
    let token = std::env::var(mcp::ENV_TOKEN).unwrap_or_default();
    Ok((base, token))
}

/// One JSON-RPC round trip to the gateway, returning the `result` or an error.
fn rpc(base: &str, token: &str, method: &str, params: Value) -> Result<Value, String> {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string();
    let (status, resp) = post(base, token, &body)?;
    if status == 401 {
        return Err("unauthorized (bad or missing gateway token)".into());
    }
    let value: Value = serde_json::from_str(&resp).map_err(|e| format!("bad response: {e}"))?;
    if let Some(err) = value.get("error") {
        let message = err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        return Err(format!("gateway error: {message}"));
    }
    Ok(value.get("result").cloned().unwrap_or(Value::Null))
}

/// POST `body` to `base`/mcp over a raw loopback connection, returning
/// `(status, body)`.
fn post(base: &str, token: &str, body: &str) -> Result<(u16, String), String> {
    let authority = base
        .trim()
        .trim_end_matches('/')
        .strip_prefix("http://")
        .ok_or("ASYLUM_MCP_URL must be http://host:port")?;
    let authority = authority.split('/').next().unwrap_or(authority);

    let mut stream =
        TcpStream::connect(authority).map_err(|e| format!("could not reach the gateway: {e}"))?;
    let mut req = format!(
        "POST /mcp HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n",
        body.len()
    );
    if !token.is_empty() {
        req.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("write failed: {e}"))?;

    let mut raw = String::new();
    stream
        .read_to_string(&mut raw)
        .map_err(|e| format!("read failed: {e}"))?;
    let (head, resp_body) = raw
        .split_once("\r\n\r\n")
        .ok_or("malformed response from gateway")?;
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .ok_or("missing status line")?;
    Ok((status, resp_body.to_string()))
}

/// Open the keep if a passphrase is available; `None` otherwise (no secrets).
fn open_keep() -> Option<keep::Keep> {
    let pass = std::env::var("ASYLUM_KEEP_PASSPHRASE")
        .ok()
        .filter(|p| !p.is_empty())?;
    let path = config::default_path().parent()?.join("keep.enc");
    path.exists()
        .then(|| keep::Keep::open(&path, &pass).ok())
        .flatten()
}
