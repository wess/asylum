//! Plain HTTP access: fetch a page as readable text, and a web search through
//! DuckDuckGo's HTML endpoint (no key needed).

use crate::clip::clip;
use anyhow::{bail, Result};
use serde::Serialize;

const AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36";

/// `proxy`: the egress proxy's port when Network Controls apply.
fn client(proxy: Option<u16>) -> Result<reqwest::Client> {
  let mut b = reqwest::Client::builder().user_agent(AGENT).timeout(std::time::Duration::from_secs(30));
  if let Some(port) = proxy {
    b = b.proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))?);
  }
  Ok(b.build()?)
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Page {
  pub url: String,
  pub title: String,
  pub text: String,
}

pub async fn fetch(url: &str, proxy: Option<u16>) -> Result<Page> {
  let url = normalize(url);
  let res = client(proxy)?.get(&url).send().await?;
  if !res.status().is_success() {
    bail!("{} returned {}", url, res.status());
  }
  let final_url = res.url().to_string();
  let kind = res
    .headers()
    .get(reqwest::header::CONTENT_TYPE)
    .and_then(|v| v.to_str().ok())
    .unwrap_or("")
    .to_string();
  let body = res.text().await?;
  let (title, text) = if kind.contains("html") || body.trim_start().starts_with('<') {
    (title_of(&body), to_text(&body))
  } else {
    (String::new(), body)
  };
  Ok(Page {
    url: final_url,
    title,
    text: clip(&text, 30_000),
  })
}

pub fn normalize(url: &str) -> String {
  let u = url.trim();
  let scheme = ["data:", "about:", "file:", "blob:", "chrome:", "view-source:"];
  if u.contains("://") || scheme.iter().any(|s| u.starts_with(s)) {
    u.to_string()
  } else {
    format!("https://{u}")
  }
}

pub fn to_text(html: &str) -> String {
  html2text::from_read(html.as_bytes(), 100).unwrap_or_default()
}

pub fn title_of(html: &str) -> String {
  let lower = html.to_lowercase();
  let Some(start) = lower.find("<title") else {
    return String::new();
  };
  let Some(open_end) = lower[start..].find('>') else {
    return String::new();
  };
  let from = start + open_end + 1;
  let Some(len) = lower[from..].find("</title>") else {
    return String::new();
  };
  decode(html[from..from + len].trim())
}

fn decode(s: &str) -> String {
  s.replace("&amp;", "&")
    .replace("&lt;", "<")
    .replace("&gt;", ">")
    .replace("&quot;", "\"")
    .replace("&#39;", "'")
    .replace("&#x27;", "'")
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Hit {
  pub title: String,
  pub url: String,
  pub snippet: String,
}

pub async fn search(query: &str, limit: usize, proxy: Option<u16>) -> Result<Vec<Hit>> {
  let url = format!(
    "https://html.duckduckgo.com/html/?q={}",
    urlencoding::encode(query)
  );
  let html = client(proxy)?.get(url).send().await?.text().await?;
  Ok(parse_results(&html, limit))
}

/// Pull results out of DuckDuckGo's HTML: each `result__a` anchor is a hit,
/// the following `result__snippet` its summary.
pub fn parse_results(html: &str, limit: usize) -> Vec<Hit> {
  let mut out = Vec::new();
  let mut rest = html;
  while let Some(i) = rest.find("class=\"result__a\"") {
    rest = &rest[i..];
    let href = attr(rest, "href=\"").unwrap_or_default();
    let title = inner(rest).unwrap_or_default();
    let snippet = rest
      .find("result__snippet")
      .and_then(|j| inner(&rest[j..]))
      .unwrap_or_default();
    out.push(Hit {
      title: strip_tags(&title),
      url: unwrap_redirect(&decode(&href)),
      snippet: strip_tags(&snippet),
    });
    if out.len() >= limit {
      break;
    }
    rest = &rest[1..];
  }
  out
}

fn attr(s: &str, key: &str) -> Option<String> {
  let start = s.find(key)? + key.len();
  let end = s[start..].find('"')?;
  Some(s[start..start + end].to_string())
}

fn inner(s: &str) -> Option<String> {
  let start = s.find('>')? + 1;
  let end = s[start..].find("</a>").or_else(|| s[start..].find("</"))?;
  Some(s[start..start + end].to_string())
}

fn strip_tags(s: &str) -> String {
  let mut out = String::new();
  let mut in_tag = false;
  for c in s.chars() {
    match c {
      '<' => in_tag = true,
      '>' => in_tag = false,
      _ if !in_tag => out.push(c),
      _ => {}
    }
  }
  decode(out.trim())
}

fn unwrap_redirect(href: &str) -> String {
  if let Some(i) = href.find("uddg=") {
    let enc = &href[i + 5..];
    let enc = enc.split('&').next().unwrap_or(enc);
    return urlencoding::decode(enc).map(|c| c.into_owned()).unwrap_or_else(|_| href.into());
  }
  if href.starts_with("//") {
    return format!("https:{href}");
  }
  href.to_string()
}

#[cfg(test)]
#[path = "../tests/web.rs"]
mod tests;

/// What a link shows on hover: its title, description, and site name.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Preview {
  pub title: String,
  pub description: String,
  pub site: String,
}

/// Read a page's Open Graph / meta tags for a hover preview.
pub async fn preview(url: &str, proxy: Option<u16>) -> Result<Preview> {
  let res = client(proxy)?.get(normalize(url)).send().await?;
  if !res.status().is_success() {
    bail!("{} returned {}", url, res.status());
  }
  // The head is enough; don't pull whole pages for a tooltip.
  let body = res.text().await?;
  Ok(parse_preview(&body[..body.len().min(200_000)]))
}

pub fn parse_preview(html: &str) -> Preview {
  let meta = |keys: &[&str]| -> String {
    let lower = html.to_lowercase();
    for key in keys {
      for attr in ["property", "name"] {
        let needle = format!("{attr}=\"{key}\"");
        let Some(at) = lower.find(&needle) else { continue };
        let start = lower[..at].rfind("<meta").unwrap_or(at);
        let end = lower[at..].find('>').map(|e| at + e).unwrap_or(lower.len());
        let tag = &html[start..end];
        if let Some(c) = tag.to_lowercase().find("content=\"") {
          let rest = &tag[c + 9..];
          if let Some(q) = rest.find('"') {
            return decode(rest[..q].trim());
          }
        }
      }
    }
    String::new()
  };
  let title = meta(&["og:title", "twitter:title"]);
  Preview {
    title: if title.is_empty() { title_of(html) } else { title },
    description: meta(&["og:description", "twitter:description", "description"]).chars().take(300).collect(),
    site: meta(&["og:site_name"]),
  }
}
