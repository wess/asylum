//! Aggregating N upstream capability listings into the one catalog the agent
//! sees, and routing a call on a namespaced name back to its upstream. Pure over
//! plain JSON, so the whole merge/route/filter/expose surface is unit-tested
//! without a live server.

use serde_json::{json, Value};

use crate::namespace;

/// One upstream's contribution to the catalog: its namespace, its per-server
/// tool filter, and the raw capability objects it listed (tool/resource/prompt
/// objects, straight from the upstream).
#[derive(Debug, Clone)]
pub struct Listing {
    pub service: String,
    /// Only expose these upstream tool names (empty = all).
    pub allow: Vec<String>,
    /// Hide these upstream tool names (applied after `allow`).
    pub deny: Vec<String>,
    pub items: Vec<Value>,
}

impl Listing {
    /// A listing with no tool filter.
    pub fn new(service: impl Into<String>, items: Vec<Value>) -> Listing {
        Listing {
            service: service.into(),
            allow: Vec::new(),
            deny: Vec::new(),
            items,
        }
    }
}

/// Whether a tool named `name` survives an `allow`/`deny` filter. `allow` is a
/// whitelist (empty = allow all); `deny` is a blacklist applied afterward.
pub fn is_allowed(name: &str, allow: &[String], deny: &[String]) -> bool {
    if !allow.is_empty() && !allow.iter().any(|a| a == name) {
        return false;
    }
    !deny.iter().any(|d| d == name)
}

/// Merge every listing's tools into one namespaced list. Each tool's `name` is
/// filtered against its server's `allow`/`deny`, then rewritten to
/// `<service>__<name>`; everything else on the tool object (description, input
/// schema) is carried through untouched.
pub fn merge_tools(listings: &[Listing]) -> Vec<Value> {
    let mut out = Vec::new();
    for listing in listings {
        for item in &listing.items {
            let Some(name) = item.get("name").and_then(Value::as_str) else {
                continue;
            };
            if !is_allowed(name, &listing.allow, &listing.deny) {
                continue;
            }
            out.push(with_mangled(item, "name", &listing.service, name));
        }
    }
    out
}

/// Merge a `field`-keyed capability (resources keyed by `uri`, prompts by
/// `name`) across listings, rewriting that field to the namespaced form. No
/// `allow`/`deny` filter - that governs tools only.
pub fn merge_field(listings: &[Listing], field: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for listing in listings {
        for item in &listing.items {
            let Some(value) = item.get(field).and_then(Value::as_str) else {
                continue;
            };
            out.push(with_mangled(item, field, &listing.service, value));
        }
    }
    out
}

/// Clone `item`, replacing `item[field]` with `mangle(service, original)`.
fn with_mangled(item: &Value, field: &str, service: &str, original: &str) -> Value {
    let mut cloned = item.clone();
    if let Some(obj) = cloned.as_object_mut() {
        obj.insert(field.into(), json!(namespace::mangle(service, original)));
    }
    cloned
}

/// Resolve a namespaced name (`<service>__<name>`) from an incoming call back to
/// `(service, upstream_name)`. Just the reverse of [`namespace::mangle`], surfaced
/// here so callers route through one vocabulary.
pub fn route(mangled: &str) -> Option<(&str, &str)> {
    namespace::split(mangled)
}

// --- Lazy exposure ("search" mode) -----------------------------------------

/// The meta-tool an agent calls to search the catalog in `search` expose mode.
pub const FIND_TOOL: &str = "asylum_find_tool";
/// The meta-tool an agent calls to invoke a tool it found, by namespaced name.
pub const CALL_TOOL: &str = "asylum_call_tool";

/// The two meta-tools advertised in `search` mode, in place of the full list.
/// The agent searches with [`FIND_TOOL`] then invokes with [`CALL_TOOL`], so tool
/// definitions load on demand instead of filling context up front.
pub fn meta_tools() -> Vec<Value> {
    vec![
        json!({
            "name": FIND_TOOL,
            "description": "Search the available tools across all connected services. \
                Returns matching tool names (namespaced as `service__tool`) and their \
                descriptions. Call this first to discover a tool, then invoke it with \
                `asylum_call_tool`.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Keywords to search tool names and descriptions for. \
                            Empty lists everything."
                    }
                }
            }
        }),
        json!({
            "name": CALL_TOOL,
            "description": "Invoke a tool discovered via `asylum_find_tool`, by its \
                namespaced name.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The namespaced tool name, e.g. `github__create_pull_request`."
                    },
                    "arguments": {
                        "type": "object",
                        "description": "The arguments to pass to the underlying tool."
                    }
                },
                "required": ["name"]
            }
        }),
    ]
}

/// Search the merged tool list for those matching `query` (case-insensitive
/// substring of the namespaced name or description), trimmed to `{name,
/// description}` and capped at `limit`. An empty query returns the first `limit`.
pub fn find(merged_tools: &[Value], query: &str, limit: usize) -> Vec<Value> {
    let needle = query.trim().to_lowercase();
    merged_tools
        .iter()
        .filter(|tool| {
            if needle.is_empty() {
                return true;
            }
            let name = tool.get("name").and_then(Value::as_str).unwrap_or("");
            let desc = tool.get("description").and_then(Value::as_str).unwrap_or("");
            name.to_lowercase().contains(&needle) || desc.to_lowercase().contains(&needle)
        })
        .take(limit)
        .map(|tool| {
            json!({
                "name": tool.get("name").cloned().unwrap_or(Value::Null),
                "description": tool.get("description").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "../tests/catalog.rs"]
mod tests;
