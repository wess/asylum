//! The upstream supervisor: the set of connected servers, the client-side
//! handshake, cached capability listings, and dispatch of a namespaced call back
//! to the server that owns it.
//!
//! A server is initialized lazily on first use (MCP `initialize` +
//! `notifications/initialized`), then its tool/resource/prompt lists are fetched
//! once and cached. Every entry point takes the caller's `project`: a
//! project-scoped server shadows a global (project `0`) one of the same service
//! name, exactly as the secrets proxy scopes upstreams. Dispatch routes purely
//! on the namespace, so routing, scoping, and filtering are all exercised against
//! a fake [`Upstream`] with no process involved.

use std::sync::Mutex;

use serde_json::{json, Value};

use crate::catalog::{self, Listing};
use crate::client::Upstream;
use crate::jsonrpc::{Payload, INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND};

/// One connected upstream plus the policy the gateway applies to it.
pub struct Server {
    pub service: String,
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    /// The project this server belongs to (`0` = global, visible everywhere).
    pub project: i64,
    upstream: Box<dyn Upstream>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// `None` until the handshake has been attempted; `Some(Ok)` once ready,
    /// `Some(Err)` if it failed (so we don't retry a dead server every call).
    ready: Option<Result<(), String>>,
    tools: Vec<Value>,
    resources: Vec<Value>,
    prompts: Vec<Value>,
}

impl Server {
    pub fn new(
        service: impl Into<String>,
        project: i64,
        allow: Vec<String>,
        deny: Vec<String>,
        upstream: Box<dyn Upstream>,
    ) -> Server {
        Server {
            service: service.into(),
            allow,
            deny,
            project,
            upstream,
            state: Mutex::new(State::default()),
        }
    }

    /// Run the handshake and cache the listings, once. Idempotent; a prior
    /// failure is remembered and returned rather than retried.
    fn ensure_ready(&self) -> Result<(), String> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(result) = &state.ready {
            return result.clone();
        }
        let result = self.handshake();
        if result.is_ok() {
            state.tools = self.list("tools/list", "tools");
            state.resources = self.list("resources/list", "resources");
            state.prompts = self.list("prompts/list", "prompts");
        }
        state.ready = Some(result.clone());
        result
    }

    fn handshake(&self) -> Result<(), String> {
        let params = json!({
            "protocolVersion": crate::PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": crate::SERVER_NAME, "version": crate::SERVER_VERSION },
        });
        let reply = self.upstream.request("initialize", params)?;
        if reply.is_error() {
            return Err(format!("upstream `{}` refused initialize", self.service));
        }
        // Per the lifecycle, the server may not be sent other requests until it
        // has received `notifications/initialized`.
        self.upstream.notify("notifications/initialized", Value::Null)?;
        Ok(())
    }

    /// Fetch a capability list; an unsupported capability (or any error) yields
    /// an empty list rather than failing the whole gateway.
    fn list(&self, method: &str, field: &str) -> Vec<Value> {
        match self.upstream.request(method, json!({})) {
            Ok(reply) => match reply.payload {
                Payload::Result(value) => value
                    .get(field)
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
                Payload::Error { .. } => Vec::new(),
            },
            Err(_) => Vec::new(),
        }
    }

    fn listing_of(&self, pick: impl Fn(&State) -> Vec<Value>) -> Listing {
        let items = {
            let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            pick(&state)
        };
        Listing {
            service: self.service.clone(),
            allow: self.allow.clone(),
            deny: self.deny.clone(),
            items,
        }
    }
}

/// The connected upstreams the gateway fronts.
pub struct Host {
    servers: Vec<Server>,
}

impl Host {
    pub fn new(servers: Vec<Server>) -> Host {
        Host { servers }
    }

    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }

    /// The distinct service names visible to `project`.
    pub fn services(&self, project: i64) -> Vec<String> {
        let mut names: Vec<String> = self
            .visible(project, None)
            .into_iter()
            .map(|s| s.service.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// The server that answers for `service` in `project`'s view: the
    /// project-scoped one if present, else the global (project `0`) one.
    fn find(&self, service: &str, project: i64) -> Option<&Server> {
        let named = |scope: i64| {
            self.servers
                .iter()
                .find(|s| s.service == service && s.project == scope)
        };
        named(project).or_else(|| named(0))
    }

    /// Every service visible to `project`, resolved to the one server that
    /// answers for it (project-scoped shadowing global). `only` restricts to a
    /// single service (the `/mcp/<service>` endpoint); `None` means all.
    fn visible(&self, project: i64, only: Option<&str>) -> Vec<&Server> {
        let mut seen: Vec<&str> = Vec::new();
        let mut out: Vec<&Server> = Vec::new();
        for server in &self.servers {
            if let Some(name) = only {
                if server.service != name {
                    continue;
                }
            }
            if seen.contains(&server.service.as_str()) {
                continue;
            }
            if let Some(chosen) = self.find(&server.service, project) {
                seen.push(&server.service);
                out.push(chosen);
            }
        }
        out
    }

    /// The merged, namespaced tool list visible to `project`. `only` scopes to
    /// one service for the `/mcp/<service>` endpoint.
    pub fn tools(&self, project: i64, only: Option<&str>) -> Vec<Value> {
        catalog::merge_tools(&self.ready_listings(project, only, |s| s.tools.clone()))
    }

    pub fn resources(&self, project: i64, only: Option<&str>) -> Vec<Value> {
        catalog::merge_field(&self.ready_listings(project, only, |s| s.resources.clone()), "uri")
    }

    pub fn prompts(&self, project: i64, only: Option<&str>) -> Vec<Value> {
        catalog::merge_field(&self.ready_listings(project, only, |s| s.prompts.clone()), "name")
    }

    /// Ensure every visible server is ready, then gather each one's listing.
    fn ready_listings(
        &self,
        project: i64,
        only: Option<&str>,
        pick: impl Fn(&State) -> Vec<Value> + Copy,
    ) -> Vec<Listing> {
        self.visible(project, only)
            .into_iter()
            .filter(|s| s.ensure_ready().is_ok())
            .map(|s| s.listing_of(pick))
            .collect()
    }

    /// Route `tools/call` on a namespaced tool name to its server.
    pub fn call_tool(&self, project: i64, name: &str, arguments: Value) -> Payload {
        self.dispatch(project, name, "tool", |server, tool| {
            if !catalog::is_allowed(tool, &server.allow, &server.deny) {
                return Err(err(METHOD_NOT_FOUND, format!("tool `{name}` is not available")));
            }
            Ok(("tools/call", json!({ "name": tool, "arguments": arguments })))
        })
    }

    /// Route `resources/read` on a namespaced uri to its server.
    pub fn read_resource(&self, project: i64, uri: &str) -> Payload {
        self.dispatch(project, uri, "uri", |_server, original| {
            Ok(("resources/read", json!({ "uri": original })))
        })
    }

    /// Route `prompts/get` on a namespaced prompt name to its server.
    pub fn get_prompt(&self, project: i64, name: &str, arguments: Value) -> Payload {
        self.dispatch(project, name, "prompt", |_server, original| {
            let mut params = json!({ "name": original });
            if !arguments.is_null() {
                params["arguments"] = arguments;
            }
            Ok(("prompts/get", params))
        })
    }

    /// Shared routing for a namespaced call: split the name, resolve the server
    /// in `project`'s view, build the upstream request via `build`, and relay the
    /// reply. `kind` names the thing for error messages.
    fn dispatch(
        &self,
        project: i64,
        mangled: &str,
        kind: &str,
        build: impl FnOnce(&Server, &str) -> Result<(&'static str, Value), Payload>,
    ) -> Payload {
        let Some((service, original)) = catalog::route(mangled) else {
            return err(INVALID_PARAMS, format!("`{mangled}` is not a namespaced {kind}"));
        };
        let Some(server) = self.find(service, project) else {
            return err(METHOD_NOT_FOUND, format!("no connected service `{service}`"));
        };
        let (method, params) = match build(server, original) {
            Ok(built) => built,
            Err(payload) => return payload,
        };
        if let Err(e) = server.ensure_ready() {
            return err(INTERNAL_ERROR, e);
        }
        relay(server.upstream.request(method, params))
    }
}

/// Turn an upstream call outcome into a payload the gateway returns: the
/// upstream's own result/error on success, an internal error on transport
/// failure.
fn relay(outcome: Result<crate::jsonrpc::Response, String>) -> Payload {
    match outcome {
        Ok(reply) => reply.payload,
        Err(e) => err(INTERNAL_ERROR, e),
    }
}

fn err(code: i64, message: impl Into<String>) -> Payload {
    Payload::Error {
        code,
        message: message.into(),
    }
}

#[cfg(test)]
#[path = "../tests/host.rs"]
mod tests;
