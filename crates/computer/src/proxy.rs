//! The computer's egress gate for Network Controls: a small HTTP proxy on
//! 127.0.0.1 that checks every destination host against a rule. The
//! browser launches through it and computer commands get it as their
//! HTTP(S)_PROXY, with the sandbox denying any other outbound connection,
//! so the rule holds for clicks, redirects, and scripts alike.

use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub type Rule = Arc<dyn Fn(&str) -> bool + Send + Sync>;

pub const BLOCKED: &str = "Blocked by your admin's network policy.";

/// Start the proxy; returns its port. It runs for the life of the process.
pub async fn start(rule: Rule) -> Result<u16> {
  let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
  let port = listener.local_addr()?.port();
  tokio::spawn(async move {
    while let Ok((sock, _)) = listener.accept().await {
      let rule = rule.clone();
      tokio::spawn(async move {
        let _ = serve(sock, rule).await;
      });
    }
  });
  Ok(port)
}

/// Where a request is going: (host, port, connect?) from its request line.
pub fn target(line: &str) -> Option<(String, u16, bool)> {
  let mut parts = line.split_whitespace();
  let method = parts.next()?;
  let uri = parts.next()?;
  if method.eq_ignore_ascii_case("CONNECT") {
    let (h, p) = uri.rsplit_once(':')?;
    return Some((h.trim_matches(['[', ']']).to_ascii_lowercase(), p.parse().ok()?, true));
  }
  let rest = uri.strip_prefix("http://")?;
  let authority = rest.split('/').next()?;
  let (h, p) = match authority.rsplit_once(':') {
    Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => (h, p.parse().ok()?),
    _ => (authority, 80),
  };
  Some((h.to_ascii_lowercase(), p, false))
}

/// `GET http://host/path HTTP/1.1` as `GET /path HTTP/1.1`.
pub fn origin_form(line: &str) -> String {
  let mut parts = line.split_whitespace();
  let (method, uri, version) = (parts.next().unwrap_or("GET"), parts.next().unwrap_or("/"), parts.next().unwrap_or("HTTP/1.1"));
  let rest = uri.strip_prefix("http://").unwrap_or(uri);
  let path = rest.find('/').map(|i| &rest[i..]).unwrap_or("/");
  format!("{method} {path} {version}")
}

async fn serve(mut client: TcpStream, rule: Rule) -> Result<()> {
  let mut head = Vec::with_capacity(2048);
  let mut buf = [0u8; 2048];
  while !head.windows(4).any(|w| w == b"\r\n\r\n") {
    let n = client.read(&mut buf).await?;
    if n == 0 || head.len() > 64 * 1024 {
      return Ok(());
    }
    head.extend_from_slice(&buf[..n]);
  }
  let text = String::from_utf8_lossy(&head).to_string();
  let line = text.lines().next().unwrap_or_default().to_string();
  let Some((host, port, connect)) = target(&line) else {
    client.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n").await?;
    return Ok(());
  };
  if !rule(&host) {
    let body = BLOCKED;
    client.write_all(format!("HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await?;
    return Ok(());
  }
  let mut upstream = TcpStream::connect((host.as_str(), port)).await?;
  if connect {
    client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await?;
    let end = head.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4).unwrap_or(head.len());
    upstream.write_all(&head[end..]).await?;
  } else {
    // Absolute-form to origin-form for the upstream server.
    let origin = origin_form(&line);
    let rest = &text[line.len()..];
    upstream.write_all(origin.as_bytes()).await?;
    upstream.write_all(rest.as_bytes()).await?;
  }
  tokio::io::copy_bidirectional(&mut client, &mut upstream).await?;
  Ok(())
}

#[cfg(test)]
#[path = "../tests/proxy.rs"]
mod tests;
