//! Building a live [`Host`] from configuration: spawn each stdio upstream,
//! construct each HTTP upstream, resolving any secrets from the keep as we go.
//! The secret-resolution and auth-formatting bits are pure and tested; the spawn
//! is the thin edge.

use config::McpServer;

use crate::client::{HttpUpstream, StdioUpstream, Upstream};
use crate::host::{Host, Server};
use crate::namespace;

const DEFAULT_HEADER: &str = "Authorization";
const DEFAULT_FORMAT: &str = "Bearer {secret}";

/// Connect every enabled server in `servers`, returning the live [`Host`] and a
/// list of human-readable warnings for the ones that were skipped (bad
/// namespace, missing command/url, failed launch). `resolve(project, name)`
/// returns a keep secret's value scoped to a project (`0` = global) - used for a
/// stdio server's `{secret:NAME}` env values and an HTTP server's auth header.
pub fn connect(
    servers: &[McpServer],
    resolve: impl Fn(i64, &str) -> Option<String>,
) -> (Host, Vec<String>) {
    let mut built = Vec::new();
    let mut warnings = Vec::new();

    for server in servers {
        if !server.enabled {
            continue;
        }
        if !namespace::is_valid_service(&server.name) {
            warnings.push(format!(
                "mcp server `{}` skipped: name must be a lowercase slug ([a-z0-9-], no `__`)",
                server.name
            ));
            continue;
        }
        match connect_one(server, &resolve, &mut warnings) {
            Some(upstream) => built.push(Server::new(
                server.name.clone(),
                server.project,
                server.allow.clone(),
                server.deny.clone(),
                upstream,
            )),
            None => continue,
        }
    }

    (Host::new(built), warnings)
}

fn connect_one(
    server: &McpServer,
    resolve: &impl Fn(i64, &str) -> Option<String>,
    warnings: &mut Vec<String>,
) -> Option<Box<dyn Upstream>> {
    match server.transport.as_str() {
        "" | "stdio" => {
            if server.command.is_empty() {
                warnings.push(format!("mcp server `{}` skipped: no command", server.name));
                return None;
            }
            let env = resolve_env(server, resolve, warnings);
            match StdioUpstream::spawn(&server.command, &server.args, &env) {
                Ok(upstream) => Some(Box::new(upstream)),
                Err(e) => {
                    warnings.push(format!("mcp server `{}` failed to launch: {e}", server.name));
                    None
                }
            }
        }
        "http" => {
            if server.url.is_empty() {
                warnings.push(format!("mcp server `{}` skipped: no url", server.name));
                return None;
            }
            let auth = build_auth(server, resolve, warnings);
            Some(Box::new(HttpUpstream::new(&server.url, auth)))
        }
        other => {
            warnings.push(format!(
                "mcp server `{}` skipped: unknown transport `{other}`",
                server.name
            ));
            None
        }
    }
}

/// Resolve a stdio server's `env` map, expanding any `{secret:NAME}` value from
/// the keep (scoped to the server's project). A literal value passes through.
fn resolve_env(
    server: &McpServer,
    resolve: &impl Fn(i64, &str) -> Option<String>,
    warnings: &mut Vec<String>,
) -> Vec<(String, String)> {
    server
        .env
        .iter()
        .map(|(key, value)| {
            let resolved = match secret_ref(value) {
                Some(name) => resolve(server.project, name).unwrap_or_else(|| {
                    warnings.push(format!(
                        "mcp server `{}`: secret `{name}` not in keep; `{key}` is empty",
                        server.name
                    ));
                    String::new()
                }),
                None => value.clone(),
            };
            (key.clone(), resolved)
        })
        .collect()
}

/// Build the auth header for an HTTP server from its `secret`, if any.
fn build_auth(
    server: &McpServer,
    resolve: &impl Fn(i64, &str) -> Option<String>,
    warnings: &mut Vec<String>,
) -> Option<(String, String)> {
    if server.secret.is_empty() {
        return None;
    }
    let Some(secret) = resolve(server.project, &server.secret).filter(|s| !s.is_empty()) else {
        warnings.push(format!(
            "mcp server `{}`: secret `{}` not in keep; sending no auth",
            server.name, server.secret
        ));
        return None;
    };
    Some(format_auth(&server.header, &server.format, &secret))
}

/// Format an auth header `(name, value)` from the configured header/format and a
/// resolved secret, applying the defaults the same way the proxy does.
fn format_auth(header: &str, format: &str, secret: &str) -> (String, String) {
    let name = non_empty(header).unwrap_or(DEFAULT_HEADER).to_string();
    let format = non_empty(format).unwrap_or(DEFAULT_FORMAT);
    (name, format.replace("{secret}", secret))
}

/// Parse a `{secret:NAME}` reference, returning `NAME`.
fn secret_ref(value: &str) -> Option<&str> {
    let inner = value.trim().strip_prefix("{secret:")?.strip_suffix('}')?;
    let name = inner.trim();
    (!name.is_empty()).then_some(name)
}

fn non_empty(s: &str) -> Option<&str> {
    let t = s.trim();
    (!t.is_empty()).then_some(t)
}

#[cfg(test)]
#[path = "../tests/setup.rs"]
mod tests;
