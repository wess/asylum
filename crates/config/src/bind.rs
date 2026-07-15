//! Bind-address safety.
//!
//! Both HTTP servers (companion, control) take a `host:port` bind string from
//! settings. An empty auth token historically meant "trust everyone", so a
//! `0.0.0.0` bind with no token silently exposed privileged endpoints to the
//! whole network. This module turns the documented rule - an unauthenticated
//! server may only listen on loopback - into an enforced [`guard`] the app runs
//! before it starts either server.
//!
//! Resolution goes through [`std::net::ToSocketAddrs`], so wildcard binds
//! (`0.0.0.0`, `[::]`), literal IPs, and hostnames are all classified after DNS
//! resolves them - `localhost` is loopback, a name pointing at a LAN address is
//! not. Anything that fails to resolve is treated as unsafe (fail closed).

use std::net::ToSocketAddrs;

/// How strict a server's bind policy is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// A non-loopback bind is allowed, but only when an auth token is set. Used
    /// by the companion server, which is meant to be reachable from a phone once
    /// the user configures a token.
    TokenGatesRemote,
    /// Only loopback binds are ever allowed, token or not. Used by the control
    /// surface, which exists solely for agents running on this machine.
    LoopbackOnly,
}

/// Why a server refused to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A non-loopback bind was requested without an authentication token.
    NonLoopbackWithoutToken(String),
    /// A non-loopback bind was requested for a loopback-only server.
    NonLoopbackNotAllowed(String),
    /// The bind address could not be resolved to any socket address.
    Unresolvable(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::NonLoopbackWithoutToken(bind) => write!(
                f,
                "refusing to bind {bind}: a non-loopback address needs an auth token; \
                 set a token or bind to 127.0.0.1"
            ),
            Refusal::NonLoopbackNotAllowed(bind) => write!(
                f,
                "refusing to bind {bind}: this server only accepts loopback binds; \
                 bind to 127.0.0.1"
            ),
            Refusal::Unresolvable(bind) => {
                write!(f, "refusing to bind {bind}: address does not resolve")
            }
        }
    }
}

/// Whether every address `bind` resolves to is a loopback address. A bind that
/// resolves to no address, or that fails to resolve, is not loopback-only.
pub fn is_loopback_only(bind: &str) -> bool {
    match bind.to_socket_addrs() {
        Ok(addrs) => {
            let mut any = false;
            for addr in addrs {
                any = true;
                if !addr.ip().is_loopback() {
                    return false;
                }
            }
            any
        }
        Err(_) => false,
    }
}

/// Decide whether a server bound at `bind` with `token`, under `policy`, may
/// start. Returns the reason it may not.
pub fn guard(bind: &str, token: &str, policy: Policy) -> Result<(), Refusal> {
    // Fail closed on anything that will not resolve, whatever the policy.
    if bind
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .is_none()
    {
        return Err(Refusal::Unresolvable(bind.to_string()));
    }
    let loopback = is_loopback_only(bind);
    match policy {
        Policy::LoopbackOnly if !loopback => Err(Refusal::NonLoopbackNotAllowed(bind.to_string())),
        Policy::TokenGatesRemote if !loopback && token.trim().is_empty() => {
            Err(Refusal::NonLoopbackWithoutToken(bind.to_string()))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "../tests/bind.rs"]
mod tests;
