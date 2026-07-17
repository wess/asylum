//! The MCP gateway.
//!
//! Asylum runs one aggregating [Model Context Protocol] server on loopback that
//! every managed agent connects to. It fronts the configured upstream MCP
//! servers ([`config::McpServer`]) under per-service namespaces: an agent opens
//! *one* connection and sees `github__create_pull_request`, `linear__create_issue`,
//! and so on, instead of configuring N servers of its own. A call to a namespaced
//! tool is routed back to the server that owns it.
//!
//! Shape mirrors the secrets [`proxy`](https://docs.rs/): the routing, merging,
//! filtering, scoping, and auth are pure and unit-tested ([`catalog`], [`token`],
//! [`namespace`], [`handle`]); the live edges - the loopback HTTP server agents
//! POST to ([`http`]) and the client that speaks to each upstream ([`client`]) -
//! are thin. The gateway is loopback-only and token-authenticated: each run's
//! token names its **project** (which upstreams it may see) and its **run** (so a
//! call is attributable). See [`SKILL`].
//!
//! Two boundaries in this first cut, both clean extension points: server→client
//! requests (sampling / elicitation) are declined rather than routed back to the
//! agent (the POST/JSON subset carries no back-channel), and a stdio upstream's
//! secrets are resolved from the keep at spawn.
//!
//! [Model Context Protocol]: https://modelcontextprotocol.io

pub mod catalog;
pub mod client;
pub mod host;
mod http;
pub mod jsonrpc;
pub mod namespace;
mod setup;
mod skill;
pub mod token;

pub use host::{Host, Server};
pub use setup::connect;
pub use skill::SKILL;

use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use jsonrpc::{Payload, Request, Response, METHOD_NOT_FOUND};

/// Base URL of the gateway, injected into each agent's environment. The MCP
/// endpoint is this URL plus `/mcp`.
pub const ENV_URL: &str = "ASYLUM_MCP_URL";
/// Per-run signed gateway token (names the run's project + run id), injected per
/// run by the app.
pub const ENV_TOKEN: &str = "ASYLUM_MCP_TOKEN";

/// The MCP protocol revision the gateway advertises.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
/// The gateway's own server identity in the MCP handshake.
pub const SERVER_NAME: &str = "asylum-gateway";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// How many matches `asylum_find_tool` returns in `search` exposure mode.
const FIND_LIMIT: usize = 50;

const MAX_CONCURRENT: usize = 8;
const RATE_MAX: u32 = 600;
const RATE_WINDOW: Duration = Duration::from_secs(10);

/// How the aggregated tools are presented to the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expose {
    /// List every upstream tool, namespaced. Simple; fills context with every
    /// tool definition.
    Direct,
    /// Advertise only `asylum_find_tool` / `asylum_call_tool`; tool definitions
    /// load on demand. Keeps a wide fleet's context small.
    Search,
}

impl Expose {
    /// Parse the `mcp.expose` setting; anything but `"search"` is `Direct`.
    pub fn parse(s: &str) -> Expose {
        if s.eq_ignore_ascii_case("search") {
            Expose::Search
        } else {
            Expose::Direct
        }
    }
}

/// One recorded tool invocation, handed to an optional audit hook so the app can
/// attribute MCP use to a run.
#[derive(Debug, Clone)]
pub struct Audit {
    pub project: i64,
    pub run: i64,
    /// The namespaced tool name the agent invoked.
    pub tool: String,
    pub ok: bool,
}

/// A side-effect sink for [`Audit`] records. The crate stays store-free; the app
/// wires this to record an event.
pub type AuditHook = Box<dyn Fn(Audit) + Send + Sync>;

/// The gateway's live configuration and connected upstreams.
pub struct Gateway {
    /// Session signing key used to verify per-run tokens. Empty disables auth
    /// and treats every request as global scope (tests only).
    pub key: String,
    /// The connected upstream servers.
    pub host: Host,
    /// How tools are exposed (direct vs. lazy search).
    pub expose: Expose,
    /// Optional per-call audit sink.
    pub audit: Option<AuditHook>,
}

impl Gateway {
    /// A gateway with no auth and no audit, for wiring a [`Host`] directly.
    pub fn new(host: Host, expose: Expose) -> Gateway {
        Gateway {
            key: String::new(),
            host,
            expose,
            audit: None,
        }
    }
}

/// Run the gateway on `addr`. Blocks; intended for its own thread. Bind safety
/// (loopback-only) is enforced by the caller.
pub fn serve(addr: impl ToSocketAddrs, gateway: Gateway) -> std::io::Result<()> {
    serve_on(TcpListener::bind(addr)?, gateway)
}

/// Serve on an already-bound listener.
pub fn serve_on(listener: TcpListener, gateway: Gateway) -> std::io::Result<()> {
    let shared = Arc::new(Shared {
        gateway,
        limiter: RateLimiter::new(RATE_WINDOW, RATE_MAX),
    });

    let (tx, rx) = mpsc::sync_channel::<TcpStream>(MAX_CONCURRENT);
    let rx = Arc::new(Mutex::new(rx));
    for _ in 0..MAX_CONCURRENT {
        let rx = Arc::clone(&rx);
        let shared = Arc::clone(&shared);
        std::thread::spawn(move || loop {
            let stream = { rx.lock().unwrap_or_else(|e| e.into_inner()).recv() };
            let Ok(mut stream) = stream else { return };
            http::handle_connection(&mut stream, |req| serve_one(&shared, req));
        });
    }

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        if tx.send(stream).is_err() {
            break;
        }
    }
    Ok(())
}

/// The address the server would bind (host:port), for display.
pub fn describe(addr: &str) -> String {
    format!("http://{addr}/mcp")
}

struct Shared {
    gateway: Gateway,
    limiter: RateLimiter,
}

/// Authenticate, scope, and dispatch one HTTP request.
fn serve_one(shared: &Shared, req: http::Request) -> http::Response {
    if req.path == "/healthz" {
        return http::Response::json(200, json!({ "ok": true }).to_string());
    }
    // The gateway lives at /mcp, with an optional /mcp/<service> to scope one
    // service. Anything else is not ours.
    let Some(rest) = req
        .path
        .split('?')
        .next()
        .and_then(|p| p.strip_prefix("/mcp"))
    else {
        return http::Response::text(404, "not found");
    };
    let only = match rest {
        "" | "/" => None,
        s => s.strip_prefix('/').filter(|s| !s.is_empty()),
    };

    // MCP over HTTP is POST-driven here; a GET would be a request to open a
    // server→client SSE stream, which this subset does not offer.
    if req.method != "POST" {
        return http::Response::text(405, "POST JSON-RPC messages to this endpoint");
    }

    let scope = match authorize(&shared.gateway.key, req.bearer.as_deref()) {
        Ok(scope) => scope,
        Err(()) => return http::Response::text(401, "unauthorized"),
    };
    if !shared.limiter.allow() {
        return http::Response::text(429, "too many requests");
    }

    let request = match Request::parse(&req.body) {
        Ok(request) => request,
        // A malformed frame still answers with a JSON-RPC error envelope (HTTP
        // 200), per JSON-RPC.
        Err(response) => return http::Response::json(200, response.to_json_string()),
    };

    match handle(&shared.gateway, scope.0, scope.1, only, request) {
        Some(response) => http::Response::json(200, response.to_json_string()),
        // A notification is owed no response.
        None => http::Response::text(202, ""),
    }
}

/// Verify the bearer token, returning `(project, run)`. An empty key disables
/// auth (global scope). A set key requires a valid, unexpired token.
fn authorize(key: &str, bearer: Option<&str>) -> Result<(i64, i64), ()> {
    if key.is_empty() {
        return Ok((0, 0));
    }
    match bearer.and_then(|b| token::verify(b, key, now())) {
        Some(scope) => Ok((scope.project, scope.run)),
        None => Err(()),
    }
}

/// Dispatch one MCP request against the gateway. Pure over the [`Gateway`]
/// (aside from the optional audit hook), so the whole method surface is tested
/// without sockets. Returns `None` for a notification (no response owed).
pub fn handle(
    gateway: &Gateway,
    project: i64,
    run: i64,
    only: Option<&str>,
    req: Request,
) -> Option<Response> {
    if req.is_notification() {
        // We consume every notification (initialized, cancelled, progress, …);
        // none needs a reply, and none needs forwarding in this subset.
        return None;
    }
    let id = req.id.clone().unwrap_or(Value::Null);
    let payload = match req.method.as_str() {
        "initialize" => Payload::Result(initialize_result()),
        "ping" => Payload::Result(json!({})),
        "logging/setLevel" => Payload::Result(json!({})),
        "tools/list" => Payload::Result(json!({ "tools": listed_tools(gateway, project, only) })),
        "tools/call" => return Some(tools_call(gateway, project, run, only, id, &req.params)),
        "resources/list" => {
            Payload::Result(json!({ "resources": gateway.host.resources(project, only) }))
        }
        "resources/templates/list" => Payload::Result(json!({ "resourceTemplates": [] })),
        "resources/read" => match req.param_str("uri") {
            Some(uri) => gateway.host.read_resource(project, uri),
            None => Payload::Error {
                code: jsonrpc::INVALID_PARAMS,
                message: "resources/read requires a uri".into(),
            },
        },
        "prompts/list" => {
            Payload::Result(json!({ "prompts": gateway.host.prompts(project, only) }))
        }
        "prompts/get" => match req.param_str("name") {
            Some(name) => {
                let args = req.params.get("arguments").cloned().unwrap_or(Value::Null);
                gateway.host.get_prompt(project, name, args)
            }
            None => Payload::Error {
                code: jsonrpc::INVALID_PARAMS,
                message: "prompts/get requires a name".into(),
            },
        },
        other => Payload::Error {
            code: METHOD_NOT_FOUND,
            message: format!("method `{other}` is not supported"),
        },
    };
    Some(Response { id, payload })
}

/// The tools the gateway lists: the two meta-tools in `search` mode, otherwise
/// every namespaced upstream tool.
fn listed_tools(gateway: &Gateway, project: i64, only: Option<&str>) -> Vec<Value> {
    match gateway.expose {
        Expose::Search => catalog::meta_tools(),
        Expose::Direct => gateway.host.tools(project, only),
    }
}

/// Handle `tools/call`, including the two `search`-mode meta-tools.
fn tools_call(
    gateway: &Gateway,
    project: i64,
    run: i64,
    only: Option<&str>,
    id: Value,
    params: &Value,
) -> Response {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    // `search` mode's discovery tool: search the catalog and return matches as a
    // text tool-result, so a plain MCP client needs nothing special.
    if gateway.expose == Expose::Search && name == catalog::FIND_TOOL {
        let query = arguments.get("query").and_then(Value::as_str).unwrap_or("");
        let tools = gateway.host.tools(project, only);
        let matches = catalog::find(&tools, query, FIND_LIMIT);
        let text = serde_json::to_string_pretty(&matches).unwrap_or_else(|_| "[]".into());
        return Response::result(id, tool_text(text));
    }

    // The tool actually being invoked: either unwrapped from `asylum_call_tool`,
    // or the namespaced name directly (allowed in either mode).
    let (target, target_args) = if gateway.expose == Expose::Search && name == catalog::CALL_TOOL {
        let inner = arguments.get("name").and_then(Value::as_str);
        let Some(inner) = inner else {
            return Response::error(
                id,
                jsonrpc::INVALID_PARAMS,
                "asylum_call_tool requires a name",
            );
        };
        let inner_args = arguments.get("arguments").cloned().unwrap_or(json!({}));
        (inner.to_string(), inner_args)
    } else if name.is_empty() {
        return Response::error(id, jsonrpc::INVALID_PARAMS, "tools/call requires a name");
    } else {
        (name.to_string(), arguments)
    };

    let payload = gateway.host.call_tool(project, &target, target_args);
    if let Some(audit) = &gateway.audit {
        audit(Audit {
            project,
            run,
            tool: target.clone(),
            ok: !matches!(payload, Payload::Error { .. }),
        });
    }
    Response { id, payload }
}

/// Wrap `text` as an MCP tool result (a single text content block).
fn tool_text(text: String) -> Value {
    json!({ "content": [ { "type": "text", "text": text } ], "isError": false })
}

/// The `initialize` result: the gateway's protocol version, capabilities, and
/// identity. Capabilities are advertised statically; a missing upstream capability
/// simply yields empty lists.
fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": { "listChanged": false },
            "resources": { "listChanged": false, "subscribe": false },
            "prompts": { "listChanged": false },
            "logging": {},
        },
        "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
        "instructions": "Aggregated MCP gateway. Tools are namespaced `service__tool`; \
            call a tool by its namespaced name.",
    })
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A fixed-window rate limiter shared across worker threads.
struct RateLimiter {
    window: Duration,
    max: u32,
    state: Mutex<(Instant, u32)>,
}

impl RateLimiter {
    fn new(window: Duration, max: u32) -> Self {
        Self {
            window,
            max,
            state: Mutex::new((Instant::now(), 0)),
        }
    }

    fn allow(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (start, count) = &mut *state;
        if Instant::now().duration_since(*start) > self.window {
            *start = Instant::now();
            *count = 0;
        }
        *count += 1;
        *count <= self.max
    }
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
