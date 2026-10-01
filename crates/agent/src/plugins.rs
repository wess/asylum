//! Connected plugins. Each installed plugin (and each account on it) is an
//! MCP server connection, opened lazily and cached. Its tools are offered to
//! Bots as `<plugin>__<tool>`; when a plugin has several accounts the tool
//! gains an `account` argument naming the label ("work", "personal").

use crate::runtime::Runtime;
use anyhow::{anyhow, bail, Result};
use mcp::http::Http;
use mcp::stdio::Stdio;
use mcp::{Client, Tokens, Tool};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use store::plugins::{self, Account, Plugin};
use tokio::sync::Mutex;

pub struct Conn {
  pub plugin: Plugin,
  pub account: Option<Account>,
  pub client: Client,
  pub tools: Vec<Tool>,
}

#[derive(Default)]
pub struct Hub {
  live: Mutex<HashMap<String, Arc<Conn>>>,
}

/// A tool as offered to a Bot.
#[derive(Clone, Debug)]
pub struct Offered {
  pub name: String,
  pub plugin: String,
  pub tool: Tool,
  pub accounts: Vec<(String, String)>,
}

pub fn slug(name: &str) -> String {
  let s: String = name
    .to_lowercase()
    .chars()
    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
    .collect();
  s.trim_matches('_').to_string()
}

fn key(plugin: &str, account: Option<&str>) -> String {
  format!("{plugin}:{}", account.unwrap_or(""))
}

impl Hub {
  pub async fn names(&self) -> Vec<String> {
    let live = self.live.lock().await;
    let mut n: Vec<String> = live
      .values()
      .map(|c| match &c.account {
        Some(a) if !a.label.is_empty() => format!("{} ({})", c.plugin.name, a.label),
        _ => c.plugin.name.clone(),
      })
      .collect();
    n.sort();
    n.dedup();
    n
  }

  pub async fn drop(&self, plugin: &str) {
    self.live.lock().await.retain(|k, _| !k.starts_with(&format!("{plugin}:")));
  }

  pub async fn connect(&self, rt: &Runtime, plugin: &Plugin, account: Option<&Account>) -> Result<Arc<Conn>> {
    let k = key(&plugin.id, account.map(|a| a.id.as_str()));
    if let Some(c) = self.live.lock().await.get(&k) {
      return Ok(c.clone());
    }
    let client = open(rt, plugin, account).await?;
    client.initialize().await?;
    let tools = client.tools().await?;
    let conn = Arc::new(Conn {
      plugin: plugin.clone(),
      account: account.cloned(),
      client,
      tools,
    });
    self.live.lock().await.insert(k, conn.clone());
    Ok(conn)
  }

  /// Every enabled tool across connected plugins.
  pub async fn offered(&self, rt: &Runtime) -> Vec<Offered> {
    let mut out: Vec<Offered> = Vec::new();
    let Ok(all) = plugins::all(&rt.pool).await else { return out };
    for p in all {
      if p.status != plugins::CONNECTED || rt.policy().blocks(Some(&p.catalog), &p.name) {
        continue;
      }
      let accounts = plugins::accounts(&rt.pool, &p.id).await.unwrap_or_default();
      let first = accounts.first();
      let conn = match self.connect(rt, &p, first).await {
        Ok(c) => c,
        Err(_) => continue,
      };
      let off = p.disabled();
      let labels: Vec<(String, String)> = accounts.iter().map(|a| (a.id.clone(), a.label.clone())).collect();
      for t in &conn.tools {
        if off.contains(&t.name) {
          continue;
        }
        let mut name = format!("{}__{}", slug(&p.name), slug(&t.name));
        name.truncate(64);
        out.push(Offered {
          name,
          plugin: p.id.clone(),
          tool: t.clone(),
          accounts: if labels.len() > 1 { labels.clone() } else { Vec::new() },
        });
      }
    }
    out
  }

  pub async fn call(&self, rt: &Runtime, offered: &Offered, mut args: Value) -> Result<(String, bool)> {
    let plugin = plugins::get(&rt.pool, &offered.plugin).await?;
    let accounts = plugins::accounts(&rt.pool, &plugin.id).await?;
    let chosen = match args.get("account").and_then(Value::as_str) {
      Some(label) => accounts
        .iter()
        .find(|a| a.label.eq_ignore_ascii_case(label))
        .or(accounts.first()),
      None => accounts.first(),
    };
    if let Some(o) = args.as_object_mut() {
      if !offered.accounts.is_empty() {
        o.remove("account");
      }
    }
    let conn = self.connect(rt, &plugin, chosen).await?;
    match conn.client.call_tool(&offered.tool.name, args.clone()).await {
      Ok(r) => Ok(r),
      Err(e) if e.downcast_ref::<mcp::http::Unauthorized>().is_some() => {
        self.drop(&plugin.id).await;
        plugins::set_status(&rt.pool, &plugin.id, plugins::NEEDS_AUTH).await?;
        rt.emit(crate::Event::Plugins);
        bail!("{} needs you to sign in again", plugin.name)
      }
      Err(e) => Err(e),
    }
  }
}

/// The JSON schema offered for a plugin tool (plus `account` when needed).
pub fn schema(o: &Offered) -> Value {
  let mut s = o.tool.input_schema.clone();
  if s.get("type").is_none() {
    s["type"] = json!("object");
  }
  if !o.accounts.is_empty() {
    let labels: Vec<&String> = o.accounts.iter().map(|(_, l)| l).collect();
    s["properties"]["account"] = json!({ "type": "string", "enum": labels, "description": "Which connected account to use" });
  }
  s
}

async fn open(rt: &Runtime, plugin: &Plugin, account: Option<&Account>) -> Result<Client> {
  let cfg = plugin.config();
  match plugin.kind.as_str() {
    plugins::COMMAND => {
      let command = cfg["command"].as_str().ok_or_else(|| anyhow!("{} has no command", plugin.name))?;
      let args: Vec<String> = cfg["args"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
      let mut env: HashMap<String, String> = cfg["env"]
        .as_object()
        .map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
        .unwrap_or_default();
      // Secret env values are stored in the keychain, named by the account.
      if let Some(a) = account {
        if let Some(json) = config::secret::get(&a.key()) {
          if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&json) {
            env.extend(map);
          }
        }
      }
      let path = std::env::var("PATH").unwrap_or_default();
      env.entry("PATH".into()).or_insert(format!("/opt/homebrew/bin:/usr/local/bin:{path}"));
      let s = Stdio::spawn(command, &args, &env, Some(&rt.computer.workspace()))?;
      Ok(Client::stdio(s))
    }
    _ => {
      let url = cfg["url"].as_str().ok_or_else(|| anyhow!("{} has no URL", plugin.name))?;
      let headers: HashMap<String, String> = cfg["headers"]
        .as_object()
        .map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
        .unwrap_or_default();
      let token = match account {
        Some(a) => token_for(a).await?,
        None => None,
      };
      Ok(Client::http(Http::new(url, headers, token)))
    }
  }
}

/// An account's bearer token, refreshing OAuth tokens that are about to
/// expire (and saving the refreshed set back to the keychain).
async fn token_for(a: &Account) -> Result<Option<String>> {
  let Some(raw) = config::secret::get(&a.key()) else { return Ok(None) };
  let Ok(mut t) = serde_json::from_str::<Tokens>(&raw) else {
    return Ok(Some(raw));
  };
  let now = store::now();
  if t.expired(now) && t.refresh_token.is_some() {
    t = mcp::oauth::refresh(&t, now).await?;
    config::secret::set(&a.key(), &serde_json::to_string(&t)?)?;
  }
  Ok(Some(t.access_token))
}

#[cfg(test)]
#[path = "../tests/plugins.rs"]
mod tests;
