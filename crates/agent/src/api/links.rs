//! Link hover previews, cached for the session. Fetched through Network
//! Controls like any other Bot web access.

use crate::runtime::Runtime;
use anyhow::Result;
use computer::web::Preview;
use std::collections::HashMap;
use std::sync::Mutex;

static CACHE: Mutex<Option<HashMap<String, Preview>>> = Mutex::new(None);

pub async fn preview(rt: &Runtime, url: &str) -> Result<Preview> {
  if let Some(p) = CACHE.lock().ok().and_then(|c| c.as_ref().and_then(|m| m.get(url).cloned())) {
    return Ok(p);
  }
  let p = computer::web::preview(url, rt.web_proxy()).await?;
  if let Ok(mut c) = CACHE.lock() {
    c.get_or_insert_with(HashMap::new).insert(url.to_string(), p.clone());
  }
  Ok(p)
}
