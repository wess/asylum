//! The Devpipe account vault, read through from Asylum.
//!
//! Nothing here is copied to this machine. The control plane stays the single
//! source of truth, and this surface is a window onto it — because two stores
//! holding the same credential have to agree about deletions, and the way that
//! fails is resurrection: a key revoked here comes back from there. A photo
//! library survives that; a credential store is defeated by it.
//!
//! The one thing that *is* stored locally is the session token, and it goes in
//! the keep — the encrypted, passphrase-locked store that exists for exactly
//! the things that should not leave the machine.
//!
//! Every call shells out to `curl` and blocks, so each action runs on the
//! background executor and posts its result back. A vault list is a network
//! round trip; doing it on the UI thread would freeze the window on a slow
//! link, which is the state a network call is most likely to be in.

use crate::secrets;

/// Where the Devpipe session token lives in the keep: global scope, since the
/// account is not per-project.
const TOKEN_NAME: &str = "DEVPIPE_SESSION";

/// What the Settings section is currently showing.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// Entries the account holds. Names and kinds only — never values.
    pub entries: Vec<devpipe::Entry>,
    /// Which boxes may read which secrets.
    pub grants: Vec<devpipe::Grant>,
    /// A value the user asked to see, keyed by `scope:scope_id:name`. Dropped
    /// when hidden rather than kept behind a flag, so it is not resident for
    /// the rest of the session.
    pub shown: std::collections::HashMap<String, String>,
    /// The last thing that went wrong, for the section to render.
    pub error: Option<String>,
    /// True while a request is in flight, so the section can say so.
    pub busy: bool,
    /// Set once a refresh has completed, so an empty list reads as "nothing
    /// here" rather than "not loaded yet".
    pub loaded: bool,
}

/// A stable key for one entry, matching the shape used by the web app.
pub fn key_of(entry: &devpipe::Entry) -> String {
    format!("{}:{}:{}", entry.scope.as_str(), entry.scope_id, entry.name)
}

/// The stored session token, if the keep is unlocked and holds one.
pub fn token() -> Option<String> {
    secrets::keep_get(0, TOKEN_NAME).filter(|t| !t.is_empty())
}

/// Whether this machine is signed in to Devpipe.
pub fn signed_in() -> bool {
    token().is_some()
}

/// A client bound to the stored token, or an explanation.
pub fn client() -> Result<devpipe::Client, String> {
    match token() {
        Some(token) => Ok(devpipe::Client::new(token).with_base(base())),
        None if secrets::keep_status() != secrets::KeepStatus::Unlocked => Err(
            "Unlock the keep to reach your Devpipe vault — the session token is kept there.".into(),
        ),
        None => Err("Sign in to Devpipe first.".into()),
    }
}

/// The instance to talk to. Overridable so a self-hosted control plane, or a
/// local one during development, works without a rebuild.
pub fn base() -> String {
    std::env::var("DEVPIPE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "https://devpipe.com/api".to_string())
}

/// Exchange credentials for a session token and put it in the keep.
///
/// The password is never stored, and never reaches argv — `devpipe::sign_in`
/// writes it to curl's stdin. What persists is the token, in the one store on
/// this machine designed to hold it.
pub fn sign_in(email: &str, password: &str) -> Result<(), String> {
    if secrets::keep_status() != secrets::KeepStatus::Unlocked {
        return Err("Unlock the keep first — the session token is stored in it.".into());
    }
    if email.trim().is_empty() || password.is_empty() {
        return Err("Enter your Devpipe email and password.".into());
    }
    let token = devpipe::sign_in(&base(), email.trim(), password).map_err(|e| e.to_string())?;
    secrets::keep_set(0, TOKEN_NAME, &token)
}

/// Forget the token. The session stays valid on the control plane until it
/// expires or is revoked there — this is "sign out of this machine", and the
/// wording in the UI should not promise more than that.
pub fn sign_out() -> Result<(), String> {
    secrets::keep_remove(0, TOKEN_NAME)
}

/// Everything the section needs in one round trip pair.
pub fn load() -> Result<(Vec<devpipe::Entry>, Vec<devpipe::Grant>), String> {
    let client = client()?;
    let entries = client.entries().map_err(|e| e.to_string())?;
    // Grants are fetched alongside rather than per-row: the section shows which
    // boxes can read each secret, and one request beats one per entry.
    let grants = client.grants().map_err(|e| e.to_string())?;
    Ok((entries, grants))
}

/// Boxes currently allowed to read `entry`.
pub fn granted_boxes<'a>(
    grants: &'a [devpipe::Grant],
    entry: &devpipe::Entry,
) -> Vec<&'a devpipe::Grant> {
    grants
        .iter()
        .filter(|g| g.scope == entry.scope && g.scope_id == entry.scope_id && g.name == entry.name)
        .collect()
}

/// A one-line summary of who can read a secret, for the row beneath it.
pub fn readable_by(grants: &[devpipe::Grant], entry: &devpipe::Entry) -> String {
    if entry.kind == devpipe::Kind::Value {
        return "Readable by your boxes".to_string();
    }
    let granted = granted_boxes(grants, entry);
    if granted.is_empty() {
        // Stated positively rather than left blank: "no grants" is the default
        // and the most important thing to be able to see at a glance.
        "No box can read this".to_string()
    } else {
        format!("Readable by {} box(es)", granted.len())
    }
}

#[cfg(test)]
#[path = "../tests/devpipevault.rs"]
mod tests;

use crate::state::Root;
use gpui::Context;

impl Root {
    /// Reload entries and grants from the control plane.
    ///
    /// The request blocks on `curl`, so it runs on the background executor and
    /// posts back. Errors land in the section rather than a toast: this is a
    /// panel you are looking at, and the failure belongs next to the thing that
    /// failed.
    pub fn devpipe_refresh(&mut self, cx: &mut Context<Self>) {
        if self.devpipe.busy {
            return;
        }
        self.devpipe.busy = true;
        self.devpipe.error = None;
        let job = cx.background_executor().spawn(async move { load() });
        cx.spawn(async move |root, cx| {
            let result = job.await;
            let _ = root.update(cx, |root, cx| {
                root.devpipe.busy = false;
                root.devpipe.loaded = true;
                match result {
                    Ok((entries, grants)) => {
                        root.devpipe.entries = entries;
                        root.devpipe.grants = grants;
                    }
                    Err(why) => root.devpipe.error = Some(why),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Sign this machine in and load the vault.
    pub fn devpipe_sign_in(&mut self, cx: &mut Context<Self>) {
        let Some(inputs) = self.settings_inputs.as_ref() else {
            return;
        };
        let email = inputs.devpipe_email.read(cx).text().to_string();
        let password = inputs.devpipe_pass.read(cx).text().to_string();
        match sign_in(&email, &password) {
            Ok(()) => {
                // Cleared immediately on success. The password has done its one
                // job and there is no reason for it to sit in a field.
                if let Some(inputs) = self.settings_inputs.as_ref() {
                    let pass = inputs.devpipe_pass.clone();
                    pass.update(cx, |input, cx| input.set_text("", cx));
                }
                self.devpipe.error = None;
                self.devpipe_refresh(cx);
            }
            Err(why) => self.devpipe.error = Some(why),
        }
        cx.notify();
    }

    /// Forget the token on this machine.
    pub fn devpipe_sign_out(&mut self, cx: &mut Context<Self>) {
        match sign_out() {
            Ok(()) => {
                self.devpipe = State::default();
            }
            Err(why) => self.devpipe.error = Some(why),
        }
        cx.notify();
    }

    /// Show or hide one entry's value.
    ///
    /// Hiding drops the plaintext instead of flipping a flag, so it is not
    /// resident for the rest of the session. Showing costs a round trip every
    /// time, which is deliberate: the control plane audits each read, and a
    /// cached value would make that record a lie.
    pub fn devpipe_toggle_value(&mut self, entry: devpipe::Entry, cx: &mut Context<Self>) {
        let key = key_of(&entry);
        if self.devpipe.shown.remove(&key).is_some() {
            cx.notify();
            return;
        }
        let client = match client() {
            Ok(client) => client,
            Err(why) => {
                self.devpipe.error = Some(why);
                cx.notify();
                return;
            }
        };
        let job = cx
            .background_executor()
            .spawn(async move { client.reveal(entry.scope, entry.scope_id, &entry.name) });
        cx.spawn(async move |root, cx| {
            let result = job.await;
            let _ = root.update(cx, |root, cx| {
                match result {
                    Ok(value) => {
                        root.devpipe.shown.insert(key, value);
                    }
                    Err(e) => root.devpipe.error = Some(e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Delete one entry from the account vault.
    pub fn devpipe_remove(&mut self, entry: devpipe::Entry, cx: &mut Context<Self>) {
        let client = match client() {
            Ok(client) => client,
            Err(why) => {
                self.devpipe.error = Some(why);
                cx.notify();
                return;
            }
        };
        let job = cx
            .background_executor()
            .spawn(async move { client.remove(entry.scope, entry.scope_id, &entry.name) });
        cx.spawn(async move |root, cx| {
            let result = job.await;
            let _ = root.update(cx, |root, cx| {
                if let Err(e) = result {
                    root.devpipe.error = Some(e.to_string());
                    cx.notify();
                } else {
                    root.devpipe_refresh(cx);
                }
            });
        })
        .detach();
    }
}
