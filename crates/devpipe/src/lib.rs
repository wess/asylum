//! The Devpipe account vault, as Asylum sees it.
//!
//! **Read through, not replicated.** Nothing here keeps a local copy of a
//! Devpipe entry. Two stores that hold the same secret have to agree about
//! deletions, and the way they fail is resurrection: a credential revoked on
//! one side comes back from the other. In a photo library that is an annoyance;
//! in a credential store it is exactly the failure the store exists to prevent.
//! So this is a client, and the control plane stays the single source of truth.
//!
//! The local [`keep`] keeps a different job: things that must never leave the
//! machine — starting with the Devpipe session token this client authenticates
//! with.
//!
//! Transport is the system `curl`, matching the `linear` and `github`
//! integrations, so there is no TLS stack in the dependency tree. Both the
//! token *and* the request body travel on curl's stdin as a `--config` file:
//! the body carries vault values and, on sign-in, a password, and `-d` would
//! put those in argv where `ps` and a crash log can read them.

use std::io::Write;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

/// A vault entry's scope. The narrowest that defines a name wins on a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Global,
    Workspace,
    Box,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Workspace => "workspace",
            Scope::Box => "box",
        }
    }
}

/// What an entry is. The distinction decides whether a box can read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Value,
    Secret,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Value => "value",
            Kind::Secret => "secret",
        }
    }
}

/// One vault entry, without its value. Listings never carry plaintext.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Entry {
    pub scope: Scope,
    pub scope_id: i64,
    pub name: String,
    pub kind: Kind,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub last_used_at: Option<String>,
}

/// A box's permission to read one secret.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Grant {
    pub box_id: i64,
    pub scope: Scope,
    pub scope_id: i64,
    pub name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not run curl: {0}")]
    Spawn(String),
    #[error("{0}")]
    Api(String),
    #[error("malformed response: {0}")]
    Protocol(String),
    #[error("not signed in to Devpipe")]
    Unauthenticated,
}

/// Mirrors the control plane's rule, so a bad name is refused without a round
/// trip and the message can say why.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    name.len() <= 64 && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Whether `url` is an http(s) endpoint — the only schemes this will call.
fn is_http_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// Escape a string for a curl config file's double-quoted value.
///
/// curl's parser understands `\\` and `\"` inside quotes; a raw quote ends the
/// value early, which would silently truncate a JSON body and send something
/// other than what was asked for. Newlines are escaped too, since a config file
/// is line-oriented and a bare newline would start a new directive.
pub fn config_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// The curl argv: everything that is *not* sensitive.
///
/// No body and no token here — both arrive on stdin. `--fail-with-body` so an
/// HTTP error is a non-zero exit whose body we can still read, which is how the
/// control plane's own message ("that is a secret, and this box has not been
/// granted it") reaches the user instead of a status code.
pub fn curl_args(method: &str, url: &str) -> Vec<String> {
    vec![
        "-sS".into(),
        "--fail-with-body".into(),
        "--connect-timeout".into(),
        "10".into(),
        "--max-time".into(),
        "30".into(),
        "-X".into(),
        method.into(),
        url.into(),
        "-H".into(),
        "Content-Type: application/json".into(),
        "--config".into(),
        "-".into(),
    ]
}

/// The curl config written to stdin: the bearer token, and the body when there
/// is one. Neither ever appears in argv.
pub fn stdin_config(token: &str, body: Option<&str>) -> String {
    let mut config = String::new();
    if !token.is_empty() {
        config.push_str(&format!(
            "header = \"Authorization: Bearer {}\"\n",
            config_escape(token)
        ));
    }
    if let Some(body) = body {
        config.push_str(&format!("data = \"{}\"\n", config_escape(body)));
    }
    config
}

/// Replace `secret` in `text` with `***`, so a token cannot leak through an
/// error. A blank secret is a no-op.
fn redact(text: &str, secret: &str) -> String {
    if secret.is_empty() {
        text.to_string()
    } else {
        text.replace(secret, "***")
    }
}

/// Pull the control plane's `{"error": "..."}` out of a failed response, so the
/// user reads the sentence that explains what to do rather than a status code.
pub fn api_error(body: &str, fallback: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
        .unwrap_or_else(|| fallback.to_string())
}

/// A client bound to one Devpipe instance and one session token.
pub struct Client {
    base: String,
    token: String,
}

impl Client {
    /// A client for `https://devpipe.com` unless told otherwise.
    pub fn new(token: impl Into<String>) -> Self {
        Client {
            base: "https://devpipe.com/api".to_string(),
            token: token.into(),
        }
    }

    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    fn send(&self, method: &str, path: &str, body: Option<&str>) -> Result<String, Error> {
        let url = format!("{}{path}", self.base);
        if !is_http_url(&url) {
            return Err(Error::Api(format!("refusing non-http(s) endpoint: {url}")));
        }
        let mut child = Command::new("curl")
            .args(curl_args(method, &url))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Spawn(e.to_string()))?;
        if let Some(mut stdin) = child.stdin.take() {
            // Dropping stdin here signals EOF to curl.
            let _ = stdin.write_all(stdin_config(&self.token, body).as_bytes());
        }
        let out = child
            .wait_with_output()
            .map_err(|e| Error::Spawn(e.to_string()))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let message = api_error(&stdout, stderr.trim());
            return Err(Error::Api(redact(&message, &self.token)));
        }
        Ok(stdout)
    }

    /// Every entry the account holds: names and kinds, never values.
    pub fn entries(&self) -> Result<Vec<Entry>, Error> {
        let body = self.send("GET", "/vault", None)?;
        serde_json::from_str(&body).map_err(|e| Error::Protocol(e.to_string()))
    }

    /// Which boxes may read which secrets.
    pub fn grants(&self) -> Result<Vec<Grant>, Error> {
        let body = self.send("GET", "/vault/grants", None)?;
        serde_json::from_str(&body).map_err(|e| Error::Protocol(e.to_string()))
    }

    /// Store an entry, replacing one of the same name in the same scope.
    pub fn put(
        &self,
        scope: Scope,
        scope_id: i64,
        name: &str,
        kind: Kind,
        value: &str,
    ) -> Result<(), Error> {
        if !valid_name(name) {
            return Err(Error::Api(
                "Names start with a letter or underscore, then letters, digits or underscores."
                    .into(),
            ));
        }
        let body = serde_json::json!({
            "scope": scope.as_str(),
            "scope_id": scope_id,
            "name": name,
            "kind": kind.as_str(),
            "value": value,
        })
        .to_string();
        self.send("POST", "/vault", Some(&body)).map(|_| ())
    }

    /// Read one entry's value. Audited on the control plane every time, so this
    /// is called on demand rather than to fill a list.
    pub fn reveal(&self, scope: Scope, scope_id: i64, name: &str) -> Result<String, Error> {
        let path = format!("/vault/{}/{scope_id}/{name}", scope.as_str());
        let body = self.send("GET", &path, None)?;
        serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("value").and_then(|s| s.as_str()).map(str::to_string))
            .ok_or_else(|| Error::Protocol("no value in response".into()))
    }

    pub fn remove(&self, scope: Scope, scope_id: i64, name: &str) -> Result<(), Error> {
        let path = format!("/vault/{}/{scope_id}/{name}", scope.as_str());
        self.send("DELETE", &path, None).map(|_| ())
    }

    /// Let one box read one secret.
    pub fn grant(&self, box_id: i64, scope: Scope, scope_id: i64, name: &str) -> Result<(), Error> {
        let body = serde_json::json!({
            "box_id": box_id,
            "scope": scope.as_str(),
            "scope_id": scope_id,
            "name": name,
        })
        .to_string();
        self.send("POST", "/vault/grants", Some(&body)).map(|_| ())
    }

    /// Withdraw it. Applies to the next read — nothing can un-read what a box
    /// already fetched.
    pub fn revoke(
        &self,
        box_id: i64,
        scope: Scope,
        scope_id: i64,
        name: &str,
    ) -> Result<(), Error> {
        let path = format!(
            "/vault/grants/{box_id}/{}/{scope_id}/{name}",
            scope.as_str()
        );
        self.send("DELETE", &path, None).map(|_| ())
    }
}

/// Exchange an email and password for a session token.
///
/// Separate from [`Client`] because it is the one call made *without* a token.
/// The password goes out on curl's stdin like everything else here; the token
/// that comes back belongs in the local keep, which is the one store on this
/// machine that should hold it.
pub fn sign_in(base: &str, email: &str, password: &str) -> Result<String, Error> {
    let url = format!("{base}/auth/login");
    if !is_http_url(&url) {
        return Err(Error::Api(format!("refusing non-http(s) endpoint: {url}")));
    }
    let body = serde_json::json!({ "email": email, "password": password }).to_string();
    let mut child = Command::new("curl")
        .args(curl_args("POST", &url))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Spawn(e.to_string()))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(stdin_config("", Some(&body)).as_bytes());
    }
    let out = child
        .wait_with_output()
        .map_err(|e| Error::Spawn(e.to_string()))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if !out.status.success() {
        // Redacted against the password, not the token: a curl error can echo
        // the config it was given.
        return Err(Error::Api(redact(
            &api_error(&stdout, "Devpipe refused those credentials."),
            password,
        )));
    }
    serde_json::from_str::<serde_json::Value>(&stdout)
        .ok()
        .and_then(|v| v.get("token").and_then(|t| t.as_str()).map(str::to_string))
        .ok_or_else(|| Error::Protocol("no token in the sign-in response".into()))
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
