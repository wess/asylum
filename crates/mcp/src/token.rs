//! Scoped, stateless gateway credentials.
//!
//! Loopback is not an authentication boundary, so every agent presents a token
//! to the gateway. The token names the caller's **project** (which scopes *which*
//! upstream servers it may see - project-scoped servers overlaid on global) and
//! its **run** (so every tool call is attributable to one run, for auditing and
//! per-run policy). It is signed with the in-memory per-session key.
//!
//! Stateless like the proxy and control tokens: `v1.<project>.<run>.<exp>.<hmac>`
//! where the HMAC is `HMAC-SHA256(key, "v1.<project>.<run>.<exp>")`.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

const VERSION: &str = "v1";

/// The authority a gateway credential carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scope {
    /// The project whose servers the holder may reach (`0` = global scope only).
    pub project: i64,
    /// The run the credential was issued to (`0` if not tied to a run).
    pub run: i64,
    /// Unix-seconds expiry; `0` means it never expires.
    pub expires_at: i64,
}

/// Mint a token for `(project, run)` signed with `key`, expiring at `expires_at`
/// unix seconds (`0` = never).
pub fn mint(key: &str, project: i64, run: i64, expires_at: i64) -> String {
    let payload = format!("{VERSION}.{project}.{run}.{expires_at}");
    format!("{payload}.{}", sign(key, &payload))
}

/// Verify a bearer token against `key` at time `now`, returning its scope if the
/// signature checks out and it has not expired.
pub fn verify(token: &str, key: &str, now: i64) -> Option<Scope> {
    let idx = token.rfind('.')?;
    let (payload, mac) = (&token[..idx], &token[idx + 1..]);
    let expected = sign(key, payload);
    // Constant-time comparison so a forged MAC leaks no timing information.
    if expected.as_bytes().ct_eq(mac.as_bytes()).unwrap_u8() != 1 {
        return None;
    }
    let mut parts = payload.split('.');
    if parts.next()? != VERSION {
        return None;
    }
    let project: i64 = parts.next()?.parse().ok()?;
    let run: i64 = parts.next()?.parse().ok()?;
    let expires_at: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    if expires_at != 0 && now >= expires_at {
        return None;
    }
    Some(Scope {
        project,
        run,
        expires_at,
    })
}

fn sign(key: &str, payload: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key.as_bytes()).expect("hmac accepts any key length");
    mac.update(payload.as_bytes());
    hex(&mac.finalize().into_bytes())
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
#[path = "../tests/token.rs"]
mod tests;
