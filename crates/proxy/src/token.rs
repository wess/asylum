//! Signed per-run proxy tokens carrying the caller's project.
//!
//! The proxy resolves secrets scoped to the requesting agent's project, so that
//! project must come from a source the agent can't forge. The app mints a token
//! bound to the run's project, signed with the per-session key
//! (`HMAC-SHA256(key, "v1.<project>.<exp>")`); the proxy verifies it and trusts
//! the extracted project. Stateless, like the control surface's tokens.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;
const VERSION: &str = "v1";

/// Mint a token for `project` signed with `key`, expiring at `expires_at` unix
/// seconds (`0` = never). Project `0` is the global scope.
pub fn mint(key: &str, project: i64, expires_at: i64) -> String {
    let payload = format!("{VERSION}.{project}.{expires_at}");
    format!("{payload}.{}", sign(key, &payload))
}

/// Verify a token against `key` at time `now`, returning the project it is bound
/// to if the signature checks out and it has not expired.
pub fn verify(token: &str, key: &str, now: i64) -> Option<i64> {
    let idx = token.rfind('.')?;
    let (payload, mac) = (&token[..idx], &token[idx + 1..]);
    let expected = sign(key, payload);
    if expected.as_bytes().ct_eq(mac.as_bytes()).unwrap_u8() != 1 {
        return None;
    }
    let mut parts = payload.split('.');
    if parts.next()? != VERSION {
        return None;
    }
    let project: i64 = parts.next()?.parse().ok()?;
    let exp: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    if exp != 0 && now >= exp {
        return None;
    }
    Some(project)
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
