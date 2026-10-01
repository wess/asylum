//! What the address bar means: a URL, a site name, a workspace file, or a
//! search. Pure, so it's unit-tested.

pub const SEARCH: &str = "https://duckduckgo.com/?q=";
pub const LOCAL: &str = "guise://localhost/";

/// Turn what was typed into a URL to load.
pub fn resolve(input: &str) -> String {
  let s = input.trim();
  if s.is_empty() {
    return "about:blank".into();
  }
  let lower = s.to_ascii_lowercase();
  if ["http://", "https://", "about:", "guise://", "data:"].iter().any(|p| lower.starts_with(p)) {
    return s.to_string();
  }
  if let Some(path) = s.strip_prefix("workspace/").or_else(|| s.strip_prefix("~/workspace/")) {
    return workspace(path);
  }
  // A host (with a dot, or localhost) and no spaces: a site.
  let host = s.split(['/', '?', '#']).next().unwrap_or("");
  let looks_like_host = !s.contains(char::is_whitespace) && (host.contains('.') || host.starts_with("localhost") || host.starts_with("127.0.0.1"));
  if looks_like_host {
    let scheme = if host.starts_with("localhost") || host.starts_with("127.0.0.1") { "http" } else { "https" };
    return format!("{scheme}://{s}");
  }
  format!("{SEARCH}{}", urlencoding::encode(s))
}

/// A workspace file's URL.
pub fn workspace(relative: &str) -> String {
  let clean: Vec<String> = relative.trim_start_matches('/').split('/').filter(|p| !p.is_empty() && *p != "." && *p != "..").map(|p| urlencoding::encode(p).into_owned()).collect();
  format!("{LOCAL}{}", clean.join("/"))
}

/// What the address bar shows for a URL.
pub fn shown(url: &str) -> String {
  match url.strip_prefix(LOCAL) {
    Some(path) => format!("workspace/{}", urlencoding::decode(path).map(|p| p.into_owned()).unwrap_or_else(|_| path.to_string())),
    None if url == "about:blank" => String::new(),
    None => url.to_string(),
  }
}

#[cfg(test)]
#[path = "../../tests/web/address.rs"]
mod tests;
