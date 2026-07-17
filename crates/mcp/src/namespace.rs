//! Namespacing so one connection can carry every upstream's capabilities without
//! collisions. A tool `create_pr` on the `github` server is presented to the
//! agent as `github__create_pr`; a call to that name is split back to
//! `(github, create_pr)` and routed to that server. The same scheme prefixes
//! resource URIs and prompt names.
//!
//! The separator is a double underscore. Service names are constrained to a
//! lowercase slug with no `__`, so the *first* `__` in a mangled name is always
//! the boundary - the underlying tool name may itself contain single (or even
//! double) underscores and still round-trips.

/// The delimiter between a service namespace and the upstream name.
pub const SEP: &str = "__";

/// Whether `name` is a valid service namespace: a non-empty lowercase slug of
/// `[a-z0-9-]`, not starting or ending with `-`, and (by construction) free of
/// the `__` separator. Enforced so mangled names split unambiguously and are
/// themselves valid MCP tool-name characters.
pub fn is_valid_service(name: &str) -> bool {
    if name.is_empty() || name.starts_with('-') || name.ends_with('-') {
        return false;
    }
    name.bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Present `name` (from `service`) to the agent, namespaced.
pub fn mangle(service: &str, name: &str) -> String {
    format!("{service}{SEP}{name}")
}

/// Split a namespaced name back into `(service, name)`, on the first separator.
/// Returns `None` if it carries no namespace.
pub fn split(mangled: &str) -> Option<(&str, &str)> {
    let idx = mangled.find(SEP)?;
    let service = &mangled[..idx];
    let name = &mangled[idx + SEP.len()..];
    if service.is_empty() || name.is_empty() {
        return None;
    }
    Some((service, name))
}

#[cfg(test)]
#[path = "../tests/namespace.rs"]
mod tests;
