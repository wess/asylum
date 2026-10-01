//! Checking Atlassian credentials before calling a connector connected.
//! mcp-atlassian starts and lists its tools even with a wrong token, so the
//! token is tried against Jira's REST API (`/rest/api/2/myself`, on Cloud
//! and Data Center alike) first.

use anyhow::{bail, Result};
use std::collections::HashMap;

/// The signed-in user's display name, or why the credentials don't work.
pub async fn verify(values: &HashMap<String, String>) -> Result<String> {
  let get = |k: &str| values.get(k).map(|v| v.trim().to_string()).unwrap_or_default();
  let site = get("JIRA_URL").trim_end_matches('/').to_string();
  if !site.starts_with("https://") && !site.starts_with("http://") {
    bail!("The Jira site URL should start with https://");
  }
  let res = reqwest::Client::new()
    .get(format!("{site}/rest/api/2/myself"))
    .basic_auth(get("JIRA_USERNAME"), Some(get("JIRA_API_TOKEN")))
    .header("Accept", "application/json")
    .timeout(std::time::Duration::from_secs(20))
    .send()
    .await
    .map_err(|e| anyhow::anyhow!("Couldn't reach {site}: {e}"))?;
  match res.status().as_u16() {
    200 => {
      let v: serde_json::Value = res.json().await.unwrap_or_default();
      Ok(v["displayName"].as_str().or(v["name"].as_str()).unwrap_or("you").to_string())
    }
    401 | 403 => bail!("Jira rejected these credentials. Check the email and API token (id.atlassian.com → Security → API tokens)."),
    404 => bail!("{site} doesn't look like a Jira site."),
    s => bail!("Jira answered {s}."),
  }
}

#[cfg(test)]
#[path = "../../tests/api/atlassian.rs"]
mod tests;
