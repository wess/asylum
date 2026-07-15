//! A dependency-light blocking HTTP client for the control API, so the CLI (and
//! an agent's `asylum control` calls) can reach the server without pulling in a
//! full HTTP stack. Talks to a `http://host:port` base over a raw `TcpStream`.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// A configured control-API client.
#[derive(Debug, Clone)]
pub struct Client {
    host: String,
    port: u16,
    token: String,
}

impl Client {
    /// Build a client from a base URL (`http://host:port`) and an optional
    /// bearer token.
    pub fn new(base_url: &str, token: impl Into<String>) -> Result<Client, String> {
        let rest = base_url
            .trim()
            .strip_prefix("http://")
            .ok_or_else(|| format!("control url must start with http:// (got `{base_url}`)"))?;
        let authority = rest.split('/').next().unwrap_or(rest);
        let (host, port) = authority
            .rsplit_once(':')
            .ok_or_else(|| format!("control url needs a port (got `{base_url}`)"))?;
        let port: u16 = port
            .parse()
            .map_err(|_| format!("bad port in `{base_url}`"))?;
        Ok(Client {
            host: host.to_string(),
            port,
            token: token.into(),
        })
    }

    /// Build a client from the environment the app injects
    /// ([`ENV_URL`](crate::ENV_URL) / [`ENV_TOKEN`](crate::ENV_TOKEN)). `None`
    /// when the URL is absent (i.e. not running inside an Asylum worktree).
    pub fn from_env() -> Option<Client> {
        let url = std::env::var(crate::ENV_URL).ok()?;
        let token = std::env::var(crate::ENV_TOKEN).unwrap_or_default();
        Client::new(&url, token).ok()
    }

    /// `GET path`, returning `(status, body)`.
    pub fn get(&self, path: &str) -> Result<(u16, String), String> {
        self.request("GET", path, None)
    }

    /// `POST path` with a JSON `body`, returning `(status, body)`.
    pub fn post(&self, path: &str, body: &str) -> Result<(u16, String), String> {
        self.request("POST", path, Some(body))
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<(u16, String), String> {
        let mut stream = TcpStream::connect((self.host.as_str(), self.port))
            .map_err(|e| format!("could not connect to control server: {e}"))?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let mut req = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
            self.host
        );
        if !self.token.is_empty() {
            req.push_str(&format!("Authorization: Bearer {}\r\n", self.token));
        }
        if let Some(body) = body {
            req.push_str("Content-Type: application/json\r\n");
            req.push_str(&format!("Content-Length: {}\r\n", body.len()));
            req.push_str("\r\n");
            req.push_str(body);
        } else {
            req.push_str("\r\n");
        }
        stream
            .write_all(req.as_bytes())
            .map_err(|e| format!("write failed: {e}"))?;

        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .map_err(|e| format!("read failed: {e}"))?;
        parse_response(&raw)
    }
}

/// Split an HTTP response into `(status, body)`.
fn parse_response(raw: &[u8]) -> Result<(u16, String), String> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or("malformed response from control server")?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or("missing status line")?;
    Ok((status, body.to_string()))
}

#[cfg(test)]
#[path = "../tests/client.rs"]
mod tests;
