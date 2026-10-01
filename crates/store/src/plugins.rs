//! Installed plugins (connectors) and the accounts connected to each.

use crate::{newid, now, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Signed in through the browser; a static key or token; a remote MCP
/// server over HTTPS; or a local MCP server run as a command.
pub const OAUTH: &str = "oauth";
pub const TOKEN: &str = "token";
pub const HTTP: &str = "http";
pub const COMMAND: &str = "command";

pub const ADDED: &str = "added";
pub const NEEDS_AUTH: &str = "needs auth";
pub const WAITING: &str = "waiting for authorization";
pub const CONNECTED: &str = "connected";
pub const DISCONNECTED: &str = "disconnected";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Plugin {
  pub id: String,
  pub catalog: String,
  pub name: String,
  pub kind: String,
  pub config: String,
  pub disabled: String,
  pub status: String,
  pub created: i64,
}

impl Plugin {
  pub fn config(&self) -> serde_json::Value {
    serde_json::from_str(&self.config).unwrap_or_default()
  }

  pub fn disabled(&self) -> Vec<String> {
    serde_json::from_str(&self.disabled).unwrap_or_default()
  }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Account {
  pub id: String,
  pub plugin_id: String,
  pub label: String,
  pub identity: String,
  pub status: String,
  pub created: i64,
}

impl Account {
  /// The keychain entry holding this account's token(s).
  pub fn key(&self) -> String {
    format!("account-{}", self.id)
  }
}

pub async fn add(pool: &Pool, catalog: &str, name: &str, kind: &str, config: &serde_json::Value) -> Result<Plugin> {
  let id = newid();
  sqlx::query("INSERT INTO plugins (id, catalog, name, kind, config, status, created) VALUES (?, ?, ?, ?, ?, ?, ?)")
    .bind(&id)
    .bind(catalog)
    .bind(name)
    .bind(kind)
    .bind(config.to_string())
    .bind(NEEDS_AUTH)
    .bind(now())
    .execute(pool)
    .await?;
  get(pool, &id).await
}

pub async fn get(pool: &Pool, id: &str) -> Result<Plugin> {
  Ok(sqlx::query_as("SELECT * FROM plugins WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn by_catalog(pool: &Pool, catalog: &str) -> Result<Option<Plugin>> {
  Ok(sqlx::query_as("SELECT * FROM plugins WHERE catalog = ?").bind(catalog).fetch_optional(pool).await?)
}

pub async fn all(pool: &Pool) -> Result<Vec<Plugin>> {
  Ok(sqlx::query_as("SELECT * FROM plugins ORDER BY name COLLATE NOCASE").fetch_all(pool).await?)
}

pub async fn set_status(pool: &Pool, id: &str, status: &str) -> Result<()> {
  sqlx::query("UPDATE plugins SET status = ? WHERE id = ?").bind(status).bind(id).execute(pool).await?;
  Ok(())
}

pub async fn set_config(pool: &Pool, id: &str, config: &serde_json::Value) -> Result<()> {
  sqlx::query("UPDATE plugins SET config = ? WHERE id = ?").bind(config.to_string()).bind(id).execute(pool).await?;
  Ok(())
}

pub async fn toggle_tool(pool: &Pool, id: &str, tool: &str, enabled: bool) -> Result<()> {
  let p = get(pool, id).await?;
  let mut off = p.disabled();
  off.retain(|t| t != tool);
  if !enabled {
    off.push(tool.to_string());
  }
  sqlx::query("UPDATE plugins SET disabled = ? WHERE id = ?")
    .bind(serde_json::to_string(&off)?)
    .bind(id)
    .execute(pool)
    .await?;
  Ok(())
}

pub async fn remove(pool: &Pool, id: &str) -> Result<()> {
  sqlx::query("DELETE FROM plugins WHERE id = ?").bind(id).execute(pool).await?;
  Ok(())
}

pub async fn add_account(pool: &Pool, plugin: &str, label: &str, identity: &str) -> Result<Account> {
  let id = newid();
  sqlx::query("INSERT INTO accounts (id, plugin_id, label, identity, created) VALUES (?, ?, ?, ?, ?)")
    .bind(&id)
    .bind(plugin)
    .bind(label)
    .bind(identity)
    .bind(now())
    .execute(pool)
    .await?;
  set_status(pool, plugin, CONNECTED).await?;
  Ok(sqlx::query_as("SELECT * FROM accounts WHERE id = ?").bind(&id).fetch_one(pool).await?)
}

pub async fn accounts(pool: &Pool, plugin: &str) -> Result<Vec<Account>> {
  Ok(sqlx::query_as("SELECT * FROM accounts WHERE plugin_id = ? ORDER BY created").bind(plugin).fetch_all(pool).await?)
}

pub async fn remove_account(pool: &Pool, id: &str) -> Result<()> {
  let plugin: Option<String> = sqlx::query_scalar("SELECT plugin_id FROM accounts WHERE id = ?")
    .bind(id)
    .fetch_optional(pool)
    .await?;
  sqlx::query("DELETE FROM accounts WHERE id = ?").bind(id).execute(pool).await?;
  if let Some(p) = plugin {
    if accounts(pool, &p).await?.is_empty() {
      set_status(pool, &p, NEEDS_AUTH).await?;
    }
  }
  Ok(())
}
