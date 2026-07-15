//! Linear integration over the Linear GraphQL API.
//!
//! Browse your teams, projects, and issues, and create an issue - the Linear
//! half of the task integration. Transport is the system `curl` (so there is
//! no async HTTP stack to carry); the [`Client`] builds the request and parses
//! the GraphQL response. The query builders and response parsers are pure and
//! tested against canned JSON - only [`Client::query`] needs the network.
//!
//! Set the API key from a personal API token (Linear → Settings → API).

mod query;

use std::io::Write;
use std::process::{Command, Stdio};

pub use query::{CREATE_ISSUE, ISSUES, PROJECTS, TEAMS};

use serde::Deserialize;

/// A Linear integration error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not run curl: {0}")]
    Spawn(String),
    #[error("linear api: {0}")]
    Api(String),
    #[error("malformed response: {0}")]
    Parse(String),
}

/// A Linear team.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Team {
    pub id: String,
    pub key: String,
    pub name: String,
}

/// A Linear project.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub state: String,
}

/// A Linear issue.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Issue {
    pub id: String,
    pub identifier: String,
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub priority: f64,
    #[serde(default, deserialize_with = "state_name")]
    pub state: String,
}

/// Issues nest workflow state as `{ "name": "..." }`; flatten to the name.
fn state_name<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    struct S {
        #[serde(default)]
        name: String,
    }
    Ok(Option::<S>::deserialize(d)?
        .map(|s| s.name)
        .unwrap_or_default())
}

/// Extract `data.<key>.nodes` from a GraphQL response and deserialize each node.
pub fn parse_nodes<T: for<'de> Deserialize<'de>>(json: &str, key: &str) -> Result<Vec<T>, Error> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| Error::Parse(e.to_string()))?;
    if let Some(errors) = value.get("errors") {
        return Err(Error::Api(errors.to_string()));
    }
    let nodes = value
        .get("data")
        .and_then(|d| d.get(key))
        .and_then(|k| k.get("nodes"))
        .cloned()
        .ok_or_else(|| Error::Parse(format!("no data.{key}.nodes")))?;
    serde_json::from_value(nodes).map_err(|e| Error::Parse(e.to_string()))
}

/// Parse a `teams` response.
pub fn parse_teams(json: &str) -> Result<Vec<Team>, Error> {
    parse_nodes(json, "teams")
}

/// Parse an `issues` response.
pub fn parse_issues(json: &str) -> Result<Vec<Issue>, Error> {
    parse_nodes(json, "issues")
}

/// Parse a `projects` response.
pub fn parse_projects(json: &str) -> Result<Vec<Project>, Error> {
    parse_nodes(json, "projects")
}

/// A Linear API client bound to a personal API token.
pub struct Client {
    api_key: String,
    endpoint: String,
}

impl Client {
    /// Create a client for the default Linear endpoint.
    pub fn new(api_key: impl Into<String>) -> Self {
        Client {
            api_key: api_key.into(),
            endpoint: "https://api.linear.app/graphql".to_string(),
        }
    }

    /// Point the client at a different endpoint (for testing/self-host).
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Run a GraphQL `query` (with optional `variables`) and return the raw
    /// JSON response body.
    ///
    /// The API token is written to curl's stdin as a `--config` header rather
    /// than passed on the command line, so it never appears in the process's
    /// argv (and thus not in `ps` or a crash log). Connection and total
    /// deadlines bound a stalled request, and any token echoed in an error is
    /// redacted.
    pub fn query(&self, query: &str, variables: serde_json::Value) -> Result<String, Error> {
        if !is_http_url(&self.endpoint) {
            return Err(Error::Api(format!(
                "refusing non-http(s) endpoint: {}",
                self.endpoint
            )));
        }
        let body = serde_json::json!({ "query": query, "variables": variables }).to_string();
        let mut child = Command::new("curl")
            .args(curl_args(&self.endpoint, &body))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Spawn(e.to_string()))?;
        if let Some(mut stdin) = child.stdin.take() {
            // Dropping stdin at the end of this block signals EOF to curl.
            let _ = stdin.write_all(auth_config(&self.api_key).as_bytes());
        }
        let out = child
            .wait_with_output()
            .map_err(|e| Error::Spawn(e.to_string()))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(Error::Api(redact(&stderr, &self.api_key)));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Fetch the teams the token can see.
    pub fn teams(&self) -> Result<Vec<Team>, Error> {
        parse_teams(&self.query(TEAMS, serde_json::Value::Null)?)
    }

    /// Fetch recent issues.
    pub fn issues(&self) -> Result<Vec<Issue>, Error> {
        parse_issues(&self.query(ISSUES, serde_json::Value::Null)?)
    }

    /// Fetch projects.
    pub fn projects(&self) -> Result<Vec<Project>, Error> {
        parse_projects(&self.query(PROJECTS, serde_json::Value::Null)?)
    }

    /// Create an issue in `team_id`. Returns the raw response for the caller to
    /// inspect (`data.issueCreate.issue`).
    pub fn create_issue(
        &self,
        team_id: &str,
        title: &str,
        description: &str,
    ) -> Result<String, Error> {
        let vars = serde_json::json!({
            "input": { "teamId": team_id, "title": title, "description": description }
        });
        self.query(CREATE_ISSUE, vars)
    }
}

/// curl argv for a request, deliberately *without* the API token: the auth
/// header is delivered on stdin (see [`auth_config`]) so it stays out of the
/// process's argv. Connection and total timeouts bound a stalled request.
fn curl_args(endpoint: &str, body: &str) -> Vec<String> {
    vec![
        "-sS".into(),
        "--connect-timeout".into(),
        "10".into(),
        "--max-time".into(),
        "30".into(),
        "-X".into(),
        "POST".into(),
        endpoint.into(),
        "-H".into(),
        "Content-Type: application/json".into(),
        "-d".into(),
        body.into(),
        // Read remaining options (the Authorization header) from stdin.
        "--config".into(),
        "-".into(),
    ]
}

/// The curl `--config` line carrying the Authorization header, written to
/// curl's stdin so the token never lands in argv.
fn auth_config(api_key: &str) -> String {
    format!("header = \"Authorization: {api_key}\"\n")
}

/// Whether `url` is an http(s) endpoint (the only schemes we will call).
fn is_http_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// Replace occurrences of `secret` in `text` with `***`, so a token cannot leak
/// through an error message. A blank secret is a no-op.
fn redact(text: &str, secret: &str) -> String {
    if secret.is_empty() {
        text.to_string()
    } else {
        text.replace(secret, "***")
    }
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
