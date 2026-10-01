//! The LiteLLM proxy's own endpoints, beside the OpenAI-compatible ones the
//! provider talks to. Teams and their model lists are how a gateway is
//! organised, so setup asks it instead of making someone type a model name.
//! Every one of these needs the key, including the team list.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Team {
  pub id: String,
  pub name: String,
  /// The models the team is scoped to. Empty means the whole catalog.
  pub models: Vec<String>,
}

/// What a key can call next to what its team allows. LiteLLM scopes a key's
/// models independently of its team's, and in a picker that just looks like
/// a gateway with fewer models, so it's worth saying.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reach {
  pub key_models: Vec<String>,
  pub team_models: Vec<String>,
}

impl Reach {
  /// Team models this key can't call (empty when the key isn't narrower).
  pub fn missing(&self) -> Vec<String> {
    if self.key_models.is_empty() {
      return Vec::new();
    }
    self.team_models.iter().filter(|m| !self.key_models.contains(m)).cloned().collect()
  }
}

/// The proxy root, without the `/v1` suffix the chat endpoint carries.
pub fn root(endpoint: &str) -> String {
  endpoint.trim_end_matches('/').trim_end_matches("/v1").trim_end_matches('/').to_string()
}

async fn get(endpoint: &str, path: &str, key: &str) -> Result<Value> {
  let res = reqwest::Client::new()
    .get(format!("{}{path}", root(endpoint)))
    .bearer_auth(key)
    .timeout(std::time::Duration::from_secs(45))
    .send()
    .await
    .with_context(|| format!("couldn't reach {}", root(endpoint)))?;
  let status = res.status();
  let text = res.text().await.unwrap_or_default();
  if !status.is_success() {
    let why = reason(&text);
    match status.as_u16() {
      401 | 403 => bail!("The gateway refused this key: {why}"),
      _ => bail!("{path} answered {status}: {why}"),
    }
  }
  serde_json::from_str(&text).with_context(|| format!("{path} didn't return JSON"))
}

/// The message out of LiteLLM's error shapes: `{"error": {"message"}}`,
/// `{"detail": {"error"}}`, or `{"detail": "..."}`.
pub fn reason(body: &str) -> String {
  serde_json::from_str::<Value>(body)
    .ok()
    .and_then(|v| v["error"]["message"].as_str().or(v["detail"]["error"].as_str()).or(v["detail"].as_str()).map(str::to_string))
    .unwrap_or_else(|| body.chars().take(200).collect())
}

/// Errors that just mean "this gateway has no teams": an older LiteLLM
/// without the endpoint, or one running without its database.
fn no_teams(e: &anyhow::Error) -> bool {
  let m = e.to_string().to_lowercase();
  m.contains("404") || m.contains("not found") || m.contains("db not connected") || m.contains("no connected db") || m.contains("no_db")
}

/// Whether the gateway accepts the key at all, before anything is saved.
pub async fn check(endpoint: &str, key: &str) -> Result<()> {
  get(endpoint, "/v1/models", key).await.map(|_| ())
}

#[derive(Deserialize)]
struct WireTeam {
  #[serde(default)]
  team_id: Option<String>,
  #[serde(default)]
  team_alias: Option<String>,
  #[serde(default)]
  models: Vec<String>,
}

/// Teams the key belongs to. LiteLLM has moved this endpoint between
/// versions, so each known path is tried and the first with teams wins. A
/// key with no team is normal.
pub async fn teams(endpoint: &str, key: &str) -> Result<Vec<Team>> {
  let mut failure: Option<anyhow::Error> = None;
  for path in ["/team/available", "/team/list", "/v1/team/list"] {
    match get(endpoint, path, key).await {
      Ok(v) => {
        let found = parse_teams(&v);
        if !found.is_empty() {
          return Ok(found);
        }
        // An endpoint that answered with no teams says so.
        return Ok(Vec::new());
      }
      // A refused key is the answer, whatever later paths say.
      Err(e) if e.to_string().contains("refused this key") => return Err(e),
      // Keep the most telling failure, not a later 404.
      Err(e) if failure.as_ref().is_none_or(no_teams) && !no_teams(&e) => failure = Some(e),
      Err(e) => {
        if failure.is_none() {
          failure = Some(e);
        }
      }
    }
  }
  match failure {
    Some(e) if !no_teams(&e) => Err(e),
    _ => Ok(Vec::new()),
  }
}

/// Team rows out of whichever shape the endpoint answered with.
pub fn parse_teams(v: &Value) -> Vec<Team> {
  let rows = match v {
    Value::Array(rows) => rows.clone(),
    Value::Object(_) => v.get("teams").or_else(|| v.get("data")).and_then(Value::as_array).cloned().unwrap_or_default(),
    _ => Vec::new(),
  };
  rows
    .into_iter()
    .filter_map(|row| {
      let w: WireTeam = serde_json::from_value(row).ok()?;
      let id = w.team_id?;
      Some(Team { name: w.team_alias.unwrap_or_else(|| id.clone()), id, models: w.models.into_iter().filter(|m| m != "all-proxy-models").collect() })
    })
    .collect()
}

/// Models to offer: the team's own list when it has one (a team scoped to
/// three models shouldn't be offered thirty), else everything the key sees.
pub async fn models(endpoint: &str, key: &str, team: Option<&Team>) -> Result<Vec<String>> {
  let mut out = match team {
    Some(t) if !t.models.is_empty() => t.models.clone(),
    _ => {
      let v = get(endpoint, "/v1/models", key).await?;
      v["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str().map(str::to_string)).collect()
    }
  };
  out.sort();
  out.dedup();
  Ok(out)
}

/// The key's own model scope (from `/key/info`) next to its team's.
pub async fn reach(endpoint: &str, key: &str, team: Option<&Team>) -> Reach {
  let key_models = match get(endpoint, "/key/info", key).await {
    Ok(v) => v["info"]["models"].as_array().into_iter().flatten().filter_map(|m| m.as_str().map(str::to_string)).filter(|m| m != "all-proxy-models").collect(),
    Err(_) => Vec::new(),
  };
  Reach { key_models, team_models: team.map(|t| t.models.clone()).unwrap_or_default() }
}

#[cfg(test)]
#[path = "../tests/litellm.rs"]
mod tests;
