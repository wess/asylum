//! Per-session secrets held in memory for the life of the process.
//!
//! The control surface's token key and the secrets-proxy signing key are
//! generated once at startup and never written to `settings.json`: they live
//! only here and in the environment of the agents the app launches, so a
//! settings live-reload never reverts them and they never land on disk.
//!
//! The **keep** is the encrypted secret store ([`keep::Keep`]); once unlocked it
//! lives behind [`keep_handle`] so the proxy resolves from it and the UI can
//! query which secrets are present, without the values ever leaving memory.

use std::sync::{Arc, Mutex, OnceLock, RwLock};

use keep::{Keep, Scope};

/// A shared handle to the (possibly still-locked) keep.
pub type SharedKeep = Arc<Mutex<Option<Keep>>>;

static CONTROL_TOKEN: OnceLock<String> = OnceLock::new();
static PROXY_KEY: OnceLock<String> = OnceLock::new();
static MCP_KEY: OnceLock<String> = OnceLock::new();
/// Rewritable, unlike the keys above: the keep can be unlocked or edited after
/// startup, and a redaction list that could only ever be set once would silently
/// stop covering every secret added afterwards.
static SECRET_VALUES: RwLock<Vec<String>> = RwLock::new(Vec::new());
static KEEP: OnceLock<SharedKeep> = OnceLock::new();

/// Record the control server's token key for this session.
pub fn set_control_token(token: String) {
    let _ = CONTROL_TOKEN.set(token);
}

/// The control server's token key for this session, or empty if disabled.
pub fn control_token() -> String {
    CONTROL_TOKEN.get().cloned().unwrap_or_default()
}

/// Record the secrets-proxy signing key (used to mint per-run project tokens).
pub fn set_proxy_key(key: String) {
    let _ = PROXY_KEY.set(key);
}

/// The secrets-proxy signing key, or empty if the proxy is disabled.
pub fn proxy_key() -> String {
    PROXY_KEY.get().cloned().unwrap_or_default()
}

/// Record the MCP gateway signing key (used to mint per-run project+run tokens).
pub fn set_mcp_key(key: String) {
    let _ = MCP_KEY.set(key);
}

/// The MCP gateway signing key, or empty if the gateway is disabled.
pub fn mcp_key() -> String {
    MCP_KEY.get().cloned().unwrap_or_default()
}

/// Record the shared keep handle (set once, at startup).
pub fn set_keep(keep: SharedKeep) {
    let _ = KEEP.set(keep);
}

/// Whether a secret named `name` is present for `project` (its own project keep
/// or the global keep). Used by the Settings surface for status.
pub fn has_secret(name: &str, project: i64) -> bool {
    let Some(handle) = KEEP.get() else {
        return false;
    };
    let guard = handle.lock().unwrap_or_else(|e| e.into_inner());
    let Some(keep) = guard.as_ref() else {
        return false;
    };
    keep.has(&Scope::Global, name) || (project != 0 && keep.has(&Scope::Project(project), name))
}

/// Snapshot the keep's secret values for transcript redaction. Call after
/// unlocking and after edits.
///
/// Very short values are skipped: masking a two-character secret would scribble
/// over ordinary output everywhere it happened to appear.
pub fn refresh_redaction_values() {
    let Some(handle) = KEEP.get() else { return };
    let guard = handle.lock().unwrap_or_else(|e| e.into_inner());
    let Some(keep) = guard.as_ref() else { return };
    let values: Vec<String> = keep
        .all_values()
        .into_iter()
        .filter(|v| v.len() >= 4)
        .collect();
    let mut slot = SECRET_VALUES.write().unwrap_or_else(|e| e.into_inner());
    *slot = values;
}

/// Replace any known secret value appearing in `text` with a mask, so a secret
/// that leaked into terminal output (e.g. an upstream that echoes it) never
/// lands in a stored transcript. Cheap when no secrets are configured.
pub fn redact(text: &str) -> String {
    let values = SECRET_VALUES.read().unwrap_or_else(|e| e.into_inner());
    if values.is_empty() {
        return text.to_string();
    }
    let mut out = text.to_string();
    for v in values.iter() {
        if out.contains(v.as_str()) {
            out = out.replace(v.as_str(), "••••••••");
        }
    }
    out
}
