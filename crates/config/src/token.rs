//! Per-session secret generation.
//!
//! The control surface can spawn agents and read transcripts, so localhost is
//! not an authentication boundary for it: any local process could otherwise
//! reach it. The app provisions a fresh random token each session and requires
//! it on every request. That token is never written back to `settings.json`; it
//! lives only in memory and in the environment of the agents the app launches.

/// Generate a strong random token: 256 bits of OS entropy as lowercase hex.
///
/// Returns `Err` only if the operating system's random source is unavailable,
/// in which case the caller should fail closed (do not start the server with a
/// guessable or empty credential).
pub fn generate() -> Result<String, getrandom::Error> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)?;
    Ok(to_hex(&bytes))
}

/// Lowercase hex encoding of `bytes`.
fn to_hex(bytes: &[u8]) -> String {
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
