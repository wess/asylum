//! MCP authorization: find the authorization server from the resource,
//! register a client dynamically, run authorization-code + PKCE through the
//! system browser with a loopback redirect, and refresh tokens.

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tokens {
  pub access_token: String,
  #[serde(default)]
  pub refresh_token: Option<String>,
  #[serde(default)]
  pub expires_at: Option<i64>,
  #[serde(default)]
  pub client_id: String,
  #[serde(default)]
  pub client_secret: Option<String>,
  #[serde(default)]
  pub token_endpoint: String,
}

impl Tokens {
  pub fn expired(&self, now_ms: i64) -> bool {
    self.expires_at.is_some_and(|t| now_ms + 60_000 >= t)
  }
}

#[derive(Clone, Debug, Default)]
pub struct Server {
  pub authorization_endpoint: String,
  pub token_endpoint: String,
  pub registration_endpoint: Option<String>,
  pub scopes: Vec<String>,
}

fn origin(url: &str) -> String {
  let after = url.find("://").map(|i| i + 3).unwrap_or(0);
  let end = url[after..].find('/').map(|i| i + after).unwrap_or(url.len());
  url[..end].to_string()
}

pub async fn discover(resource: &str, metadata_url: Option<&str>) -> Result<Server> {
  let http = reqwest::Client::new();
  let meta_url = metadata_url
    .map(str::to_string)
    .unwrap_or_else(|| format!("{}/.well-known/oauth-protected-resource", origin(resource)));
  let issuer = match http.get(&meta_url).send().await {
    Ok(r) if r.status().is_success() => r
      .json::<Value>()
      .await
      .ok()
      .and_then(|v| v["authorization_servers"][0].as_str().map(str::to_string))
      .unwrap_or_else(|| origin(resource)),
    _ => origin(resource),
  };
  let base = issuer.trim_end_matches('/');
  let path_suffix = base.strip_prefix(&origin(base)).unwrap_or("");
  let candidates = [
    format!("{}/.well-known/oauth-authorization-server{path_suffix}", origin(base)),
    format!("{}/.well-known/openid-configuration{path_suffix}", origin(base)),
    format!("{base}/.well-known/oauth-authorization-server"),
  ];
  for url in candidates {
    if let Ok(r) = http.get(&url).send().await {
      if r.status().is_success() {
        let v: Value = r.json().await?;
        return Ok(Server {
          authorization_endpoint: v["authorization_endpoint"].as_str().unwrap_or_default().into(),
          token_endpoint: v["token_endpoint"].as_str().unwrap_or_default().into(),
          registration_endpoint: v["registration_endpoint"].as_str().map(str::to_string),
          scopes: v["scopes_supported"]
            .as_array()
            .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
            .unwrap_or_default(),
        });
      }
    }
  }
  bail!("could not find this server's sign-in endpoints")
}

pub async fn register(server: &Server, redirect: &str) -> Result<(String, Option<String>)> {
  let endpoint = server
    .registration_endpoint
    .as_ref()
    .ok_or_else(|| anyhow!("this server does not support automatic client registration"))?;
  let body = serde_json::json!({
    "client_name": "Asylum",
    "redirect_uris": [redirect],
    "grant_types": ["authorization_code", "refresh_token"],
    "response_types": ["code"],
    "token_endpoint_auth_method": "none"
  });
  let v: Value = reqwest::Client::new().post(endpoint).json(&body).send().await?.error_for_status()?.json().await?;
  let id = v["client_id"].as_str().ok_or_else(|| anyhow!("registration returned no client id"))?;
  Ok((id.to_string(), v["client_secret"].as_str().map(str::to_string)))
}

pub fn pkce() -> (String, String) {
  let bytes: [u8; 32] = rand::rng().random();
  let verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
  let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()));
  (verifier, challenge)
}

pub fn state() -> String {
  let bytes: [u8; 16] = rand::rng().random();
  base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub struct Pending {
  pub url: String,
  pub verifier: String,
  pub state: String,
  pub redirect: String,
  pub listener: tokio::net::TcpListener,
}

/// Bind a loopback port and build the authorization URL to open.
pub async fn begin(server: &Server, client_id: &str, resource: &str, scopes: &[String]) -> Result<Pending> {
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
  let port = listener.local_addr()?.port();
  let redirect = format!("http://127.0.0.1:{port}/callback");
  let (verifier, challenge) = pkce();
  let st = state();
  let mut url = format!(
    "{}?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256&state={}&resource={}",
    server.authorization_endpoint,
    urlencoding::encode(client_id),
    urlencoding::encode(&redirect),
    challenge,
    st,
    urlencoding::encode(resource)
  );
  if !scopes.is_empty() {
    url.push_str(&format!("&scope={}", urlencoding::encode(&scopes.join(" "))));
  }
  Ok(Pending { url, verifier, state: st, redirect, listener })
}

/// A loopback redirect URI for registration before `begin` binds: the port
/// is fixed up by registering the pattern the server allows.
pub async fn loopback() -> Result<(tokio::net::TcpListener, String)> {
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
  let port = listener.local_addr()?.port();
  Ok((listener, format!("http://127.0.0.1:{port}/callback")))
}

/// Wait (up to `timeout`) for the browser to hit the redirect, returning the
/// authorization code.
pub async fn wait_code(listener: &tokio::net::TcpListener, state: &str, timeout: std::time::Duration) -> Result<String> {
  let fut = async {
    loop {
      let (mut sock, _) = listener.accept().await?;
      let mut buf = vec![0u8; 8192];
      let n = sock.read(&mut buf).await?;
      let req = String::from_utf8_lossy(&buf[..n]).to_string();
      let path = req.split_whitespace().nth(1).unwrap_or("").to_string();
      let q = parse_query(&path);
      let body = if q.contains_key("code") {
        "<html><body style=\"font-family:system-ui;padding:40px\"><h2>Connected</h2><p>You can close this tab and return to Asylum.</p></body></html>"
      } else {
        "<html><body style=\"font-family:system-ui;padding:40px\"><h2>Sign-in did not finish</h2><p>Return to Asylum and try again.</p></body></html>"
      };
      let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
      let _ = sock.write_all(resp.as_bytes()).await;
      if let Some(err) = q.get("error") {
        bail!("sign-in failed: {err}");
      }
      if let Some(code) = q.get("code") {
        if q.get("state").map(String::as_str) != Some(state) {
          bail!("sign-in state mismatch");
        }
        return Ok(code.clone());
      }
    }
  };
  tokio::time::timeout(timeout, fut).await.context("sign-in timed out")?
}

pub fn parse_query(path: &str) -> std::collections::HashMap<String, String> {
  let q = path.split_once('?').map(|(_, q)| q).unwrap_or("");
  q.split('&')
    .filter_map(|kv| kv.split_once('='))
    .map(|(k, v)| (k.to_string(), urlencoding::decode(v).map(|c| c.into_owned()).unwrap_or_default()))
    .collect()
}

#[allow(clippy::too_many_arguments)]
pub async fn exchange(server: &Server, client_id: &str, secret: Option<&str>, code: &str, verifier: &str, redirect: &str, resource: &str, now_ms: i64) -> Result<Tokens> {
  let mut form = vec![
    ("grant_type", "authorization_code".to_string()),
    ("code", code.to_string()),
    ("redirect_uri", redirect.to_string()),
    ("client_id", client_id.to_string()),
    ("code_verifier", verifier.to_string()),
    ("resource", resource.to_string()),
  ];
  if let Some(s) = secret {
    form.push(("client_secret", s.to_string()));
  }
  token_request(&server.token_endpoint, &form, client_id, secret, now_ms).await
}

pub async fn refresh(t: &Tokens, now_ms: i64) -> Result<Tokens> {
  let rt = t.refresh_token.clone().ok_or_else(|| anyhow!("no refresh token"))?;
  let mut form = vec![
    ("grant_type", "refresh_token".to_string()),
    ("refresh_token", rt.clone()),
    ("client_id", t.client_id.clone()),
  ];
  if let Some(s) = &t.client_secret {
    form.push(("client_secret", s.clone()));
  }
  let mut next = token_request(&t.token_endpoint, &form, &t.client_id, t.client_secret.as_deref(), now_ms).await?;
  if next.refresh_token.is_none() {
    next.refresh_token = Some(rt);
  }
  Ok(next)
}

async fn token_request(endpoint: &str, form: &[(&str, String)], client_id: &str, secret: Option<&str>, now_ms: i64) -> Result<Tokens> {
  let res = reqwest::Client::new()
    .post(endpoint)
    .header("Accept", "application/json")
    .form(form)
    .send()
    .await?;
  if !res.status().is_success() {
    let body = res.text().await.unwrap_or_default();
    bail!("token request failed: {body}");
  }
  let v: Value = res.json().await?;
  Ok(Tokens {
    access_token: v["access_token"].as_str().ok_or_else(|| anyhow!("no access token"))?.to_string(),
    refresh_token: v["refresh_token"].as_str().map(str::to_string),
    expires_at: v["expires_in"].as_i64().map(|s| now_ms + s * 1000),
    client_id: client_id.to_string(),
    client_secret: secret.map(str::to_string),
    token_endpoint: endpoint.to_string(),
  })
}

#[cfg(test)]
#[path = "../tests/oauth.rs"]
mod tests;
