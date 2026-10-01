//! Adding plugins and connecting accounts: OAuth in the system browser
//! (MCP authorization with dynamic registration and PKCE), a pasted token,
//! or a command's environment.

use crate::catalog;
use crate::event::Event;
use crate::runtime::Runtime;
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use store::plugins::{self, Plugin};

pub const SIGN_IN_WINDOW: Duration = Duration::from_secs(600);

pub async fn add(rt: &Runtime, id: &str) -> Result<Plugin> {
  if let Some(p) = plugins::by_catalog(&rt.pool, id).await? {
    return Ok(p);
  }
  let e = catalog::plugin(id).ok_or_else(|| anyhow!("unknown plugin"))?;
  if rt.policy().blocks(Some(e.id), e.name) {
    bail!("Disabled by your admin.");
  }
  let cfg = json!({
    "url": e.url,
    "command": e.command,
    "args": e.args,
    "header": e.header,
    "fields": e.fields.iter().map(|f| json!({"name": f.name, "label": f.label, "secret": f.secret, "optional": f.optional})).collect::<Vec<_>>(),
    "multi_account": e.multi_account,
    "description": e.description,
  });
  let p = plugins::add(&rt.pool, e.id, e.name, e.kind, &cfg).await?;
  crate::audit::change(rt, "user", "plugin.added", e.name, e.id).await;
  // Open remote servers and field-less commands need no sign-in.
  if e.kind == plugins::HTTP || (e.kind == plugins::COMMAND && e.fields.is_empty()) {
    plugins::set_status(&rt.pool, &p.id, plugins::CONNECTED).await?;
  }
  rt.emit(Event::Plugins);
  plugins::get(&rt.pool, &p.id).await
}

/// A custom MCP server: a remote HTTPS URL or a local command.
pub async fn add_custom(rt: &Runtime, name: &str, url: Option<&str>, command: Option<&str>, args: &[String], oauth: bool) -> Result<Plugin> {
  let (kind, cfg) = match (url, command) {
    (Some(u), _) if !u.trim().is_empty() => {
      if !u.starts_with("https://") && !u.starts_with("http://127.0.0.1") && !u.starts_with("http://localhost") {
        bail!("Remote servers must use HTTPS.");
      }
      (if oauth { plugins::OAUTH } else { plugins::HTTP }, json!({ "url": u, "multi_account": oauth }))
    }
    (_, Some(c)) if !c.trim().is_empty() => (plugins::COMMAND, json!({ "command": c, "args": args })),
    _ => bail!("Give a URL or a command."),
  };
  let p = plugins::add(&rt.pool, "", name, kind, &cfg).await?;
  if kind == plugins::HTTP {
    plugins::set_status(&rt.pool, &p.id, plugins::CONNECTED).await?;
  }
  rt.emit(Event::Plugins);
  plugins::get(&rt.pool, &p.id).await
}

/// Begin an OAuth sign-in. Returns the URL to open; completion happens in
/// the background (within the sign-in window) and updates the plugin.
pub async fn authorize(rt: &Runtime, plugin: &str, label: &str) -> Result<String> {
  let p = plugins::get(&rt.pool, plugin).await?;
  let url = p.config()["url"].as_str().unwrap_or_default().to_string();
  let server = mcp::oauth::discover(&url, None).await?;
  let (listener, redirect) = mcp::oauth::loopback().await?;
  let (client_id, secret) = mcp::oauth::register(&server, &redirect).await?;
  let (verifier, challenge) = mcp::oauth::pkce();
  let state = mcp::oauth::state();
  let mut auth = format!(
    "{}?response_type=code&client_id={}&redirect_uri={}&code_challenge={challenge}&code_challenge_method=S256&state={state}&resource={}",
    server.authorization_endpoint,
    enc(&client_id),
    enc(&redirect),
    enc(&url)
  );
  if !server.scopes.is_empty() {
    auth.push_str(&format!("&scope={}", enc(&server.scopes.join(" "))));
  }
  plugins::set_status(&rt.pool, plugin, plugins::WAITING).await?;
  rt.emit(Event::Plugins);
  let rt2 = rt.clone();
  let (pid, label) = (plugin.to_string(), label.to_string());
  tokio::spawn(async move {
    let result = async {
      let code = mcp::oauth::wait_code(&listener, &state, SIGN_IN_WINDOW).await?;
      let tokens = mcp::oauth::exchange(&server, &client_id, secret.as_deref(), &code, &verifier, &redirect, &url, store::now()).await?;
      let acct = plugins::add_account(&rt2.pool, &pid, &label, "").await?;
      config::secret::set(&acct.key(), &serde_json::to_string(&tokens)?)?;
      rt2.plugins.drop(&pid).await;
      anyhow::Ok(())
    }
    .await;
    if result.is_err() {
      let has = plugins::accounts(&rt2.pool, &pid).await.map(|a| !a.is_empty()).unwrap_or(false);
      let _ = plugins::set_status(&rt2.pool, &pid, if has { plugins::CONNECTED } else { plugins::NEEDS_AUTH }).await;
    }
    rt2.emit(Event::Plugins);
  });
  Ok(auth)
}

fn enc(s: &str) -> String {
  s.bytes()
    .map(|b| match b {
      b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
      _ => format!("%{b:02X}"),
    })
    .collect()
}

/// Connect with a token (token plugins) or environment values (commands).
pub async fn connect_fields(rt: &Runtime, plugin: &str, label: &str, mut values: HashMap<String, String>) -> Result<()> {
  let p = plugins::get(&rt.pool, plugin).await?;
  let fields: Vec<Value> = p.config()["fields"].as_array().cloned().unwrap_or_default();
  for f in &fields {
    let name = f["name"].as_str().unwrap_or("");
    if values.get(name).is_none_or(|v| v.trim().is_empty()) {
      if f["optional"].as_bool().unwrap_or(false) {
        values.remove(name);
        continue;
      }
      bail!("{} is required", f["label"].as_str().unwrap_or(name));
    }
  }
  // Some servers start with any credentials; check them where we can.
  if p.catalog == "jiratoken" {
    crate::api::atlassian::verify(&values).await?;
  }
  let acct = plugins::add_account(&rt.pool, plugin, label, "").await?;
  let secret = if p.kind == plugins::TOKEN {
    values.get("token").cloned().unwrap_or_default()
  } else {
    serde_json::to_string(&values)?
  };
  config::secret::set(&acct.key(), &secret)?;
  rt.plugins.drop(plugin).await;
  // Prove it works before calling it connected.
  if let Err(e) = rt.plugins.connect(rt, &p, Some(&acct)).await {
    plugins::remove_account(&rt.pool, &acct.id).await?;
    let _ = config::secret::remove(&acct.key());
    rt.emit(Event::Plugins);
    bail!("Couldn't connect: {e}");
  }
  rt.emit(Event::Plugins);
  Ok(())
}

pub async fn remove_account(rt: &Runtime, plugin: &str, account: &str) -> Result<()> {
  let _ = config::secret::remove(&format!("account-{account}"));
  plugins::remove_account(&rt.pool, account).await?;
  rt.plugins.drop(plugin).await;
  rt.emit(Event::Plugins);
  Ok(())
}

pub async fn remove(rt: &Runtime, plugin: &str) -> Result<()> {
  let p = plugins::get(&rt.pool, plugin).await?;
  if crate::policy::required(rt, &p.catalog) {
    bail!("Required by your admin.");
  }
  crate::audit::change(rt, "user", "plugin.removed", &p.name, &p.catalog).await;
  for a in plugins::accounts(&rt.pool, plugin).await? {
    let _ = config::secret::remove(&a.key());
  }
  plugins::remove(&rt.pool, plugin).await?;
  rt.plugins.drop(plugin).await;
  rt.emit(Event::Plugins);
  Ok(())
}

pub async fn toggle_tool(rt: &Runtime, plugin: &str, tool: &str, on: bool) -> Result<()> {
  plugins::toggle_tool(&rt.pool, plugin, tool, on).await?;
  rt.emit(Event::Plugins);
  Ok(())
}

/// Tools a connected plugin offers (connecting if needed).
pub async fn tools(rt: &Runtime, plugin: &str) -> Result<Vec<mcp::Tool>> {
  let p = plugins::get(&rt.pool, plugin).await?;
  let accts = plugins::accounts(&rt.pool, plugin).await?;
  let conn = rt.plugins.connect(rt, &p, accts.first()).await?;
  Ok(conn.tools.clone())
}
