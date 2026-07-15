//! Scoped, stateless control credentials.
//!
//! Localhost is not an authentication boundary for the control surface, so every
//! request must present a token. Beyond merely proving knowledge of the session
//! key, a token is *scoped*: it names the task and run it was issued for, signed
//! with the in-memory session key. The server recomputes the signature and,
//! having the caller's task, refuses operations on any other task - so a single
//! compromised agent cannot reach across the fleet.
//!
//! The token is stateless: `v1.<task>.<run>.<exp>.<hex-hmac>` where the HMAC is
//! `HMAC-SHA256(key, "v1.<task>.<run>.<exp>")`. Nothing is persisted, and the
//! raw key never leaves memory.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

const VERSION: &str = "v1";

/// The authority a control credential carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scope {
    /// The task the holder may operate on (its own and its siblings' runs).
    pub task_id: i64,
    /// The run that was issued this credential.
    pub run_id: i64,
    /// Unix-seconds expiry; `0` means it never expires.
    pub expires_at: i64,
}

/// Mint a scoped token for `(task_id, run_id)` signed with `key`, expiring at
/// `expires_at` unix seconds (`0` = never).
pub fn mint(key: &str, task_id: i64, run_id: i64, expires_at: i64) -> String {
    let payload = format!("{VERSION}.{task_id}.{run_id}.{expires_at}");
    let mac = sign(key, &payload);
    format!("{payload}.{mac}")
}

/// Verify a bearer token against `key` at time `now`, returning its scope if the
/// signature checks out and it has not expired.
pub fn verify(token: &str, key: &str, now: i64) -> Option<Scope> {
    let idx = token.rfind('.')?;
    let (payload, mac_hex) = (&token[..idx], &token[idx + 1..]);
    let expected = sign(key, payload);
    // Constant-time comparison so a forged MAC leaks no timing information.
    if expected.as_bytes().ct_eq(mac_hex.as_bytes()).unwrap_u8() != 1 {
        return None;
    }
    let mut parts = payload.split('.');
    if parts.next()? != VERSION {
        return None;
    }
    let task_id: i64 = parts.next()?.parse().ok()?;
    let run_id: i64 = parts.next()?.parse().ok()?;
    let expires_at: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    if expires_at != 0 && now >= expires_at {
        return None;
    }
    Some(Scope {
        task_id,
        run_id,
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
