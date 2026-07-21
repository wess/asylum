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

// ── In-app keep unlock + secret management ──────────────────────────────────
//
// The keep is opened at startup only when the proxy is on (see `main`); this
// lets the Settings surface unlock it on demand and manage secrets the same
// way the `asylum keep` CLI does. The passphrase is only ever borrowed to
// derive the key and is never held here; secret values go into the encrypted
// keep and never into settings.json. Every read/write goes through the shared
// keep handle - never the Root entity - so it is safe to call during a render.

/// Whether the keep exists and whether it is currently unlocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepStatus {
    /// No keep file on disk yet; a passphrase would create one.
    Missing,
    /// A keep file exists but is not unlocked this session.
    Locked,
    /// Unlocked and held in memory.
    Unlocked,
}

/// The keep file, alongside `settings.json` (matches the CLI and `main`).
fn keep_file_path() -> std::path::PathBuf {
    config::default_path()
        .parent()
        .map(|dir| dir.join("keep.enc"))
        .unwrap_or_else(|| std::path::PathBuf::from("keep.enc"))
}

fn scope_of(project: i64) -> Scope {
    if project == 0 {
        Scope::Global
    } else {
        Scope::Project(project)
    }
}

/// The current keep status for the Settings surface.
pub fn keep_status() -> KeepStatus {
    let unlocked = KEEP
        .get()
        .map(|h| h.lock().unwrap_or_else(|e| e.into_inner()).is_some())
        .unwrap_or(false);
    if unlocked {
        KeepStatus::Unlocked
    } else if keep_file_path().exists() {
        KeepStatus::Locked
    } else {
        KeepStatus::Missing
    }
}

/// Unlock (or, when no file exists yet, create and persist) the keep with
/// `passphrase`, holding it in the shared handle. The passphrase is not
/// retained. A freshly created keep is saved empty so its passphrase is
/// established on disk before any secret is added.
pub fn unlock_keep(passphrase: &str) -> Result<(), String> {
    if passphrase.is_empty() {
        return Err("Enter a passphrase.".into());
    }
    let Some(handle) = KEEP.get() else {
        return Err("The keep is not initialized.".into());
    };
    let path = keep_file_path();
    let opened = if path.exists() {
        Keep::open(&path, passphrase).map_err(|e| e.to_string())?
    } else {
        let keep = Keep::create(passphrase).map_err(|e| e.to_string())?;
        keep.save(&path).map_err(|e| e.to_string())?;
        keep
    };
    *handle.lock().unwrap_or_else(|e| e.into_inner()) = Some(opened);
    refresh_redaction_values();
    Ok(())
}

/// Every scope that holds a secret, each with its sorted secret names (values
/// are never returned). Empty when the keep is locked or absent.
pub fn keep_scopes() -> Vec<(String, Vec<String>)> {
    let Some(handle) = KEEP.get() else {
        return Vec::new();
    };
    let guard = handle.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().map(|k| k.scopes()).unwrap_or_default()
}

/// Set `name` to `value` in the keep, scoped Global (`project == 0`) or to a
/// project, and persist atomically. Requires an unlocked keep.
pub fn keep_set(project: i64, name: &str, value: &str) -> Result<(), String> {
    let Some(handle) = KEEP.get() else {
        return Err("The keep is not initialized.".into());
    };
    let mut guard = handle.lock().unwrap_or_else(|e| e.into_inner());
    let Some(keep) = guard.as_mut() else {
        return Err("Unlock the keep first.".into());
    };
    keep.set(&scope_of(project), name, value);
    keep.save(&keep_file_path()).map_err(|e| e.to_string())?;
    drop(guard);
    refresh_redaction_values();
    Ok(())
}

/// Remove `name` from the keep in the given scope and persist. Requires an
/// unlocked keep.
pub fn keep_remove(project: i64, name: &str) -> Result<(), String> {
    let Some(handle) = KEEP.get() else {
        return Err("The keep is not initialized.".into());
    };
    let mut guard = handle.lock().unwrap_or_else(|e| e.into_inner());
    let Some(keep) = guard.as_mut() else {
        return Err("Unlock the keep first.".into());
    };
    if !keep.remove(&scope_of(project), name) {
        return Err(format!("No such secret: {name}"));
    }
    keep.save(&keep_file_path()).map_err(|e| e.to_string())?;
    drop(guard);
    refresh_redaction_values();
    Ok(())
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
