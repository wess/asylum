//! Sections sync across Macs through a shared folder (iCloud Drive by
//! default). Each device writes `sections.json` when its sections or Bot
//! placement change, and applies a newer file written by another device.
//! Bots are matched by name, sections by id.

use crate::event::Event;
use crate::runtime::Runtime;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const FILE: &str = "sections.json";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Doc {
  pub device: String,
  pub updated: i64,
  pub sections: Vec<Section>,
  /// Bot name → section id.
  pub placement: BTreeMap<String, String>,
  /// Pinned Bot names.
  pub pinned: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Section {
  pub id: String,
  pub name: String,
  pub position: i64,
}

pub fn icloud() -> PathBuf {
  let home = std::env::var("HOME").unwrap_or_default();
  PathBuf::from(home).join("Library/Mobile Documents/com~apple~CloudDocs/Asylum")
}

async fn device(rt: &Runtime) -> String {
  if let Ok(Some(d)) = store::state::get(&rt.pool, "device").await {
    return d;
  }
  let d = store::newid();
  let _ = store::state::set(&rt.pool, "device", &d).await;
  d
}

/// This device's sections as a document (without a timestamp).
pub async fn local(rt: &Runtime) -> Result<Doc> {
  let sections = store::sections::all(&rt.pool).await?;
  let bots = store::bots::list(&rt.pool).await?;
  Ok(Doc {
    device: device(rt).await,
    updated: 0,
    sections: sections.into_iter().map(|s| Section { id: s.id, name: s.name, position: s.position }).collect(),
    placement: bots.iter().filter_map(|b| b.section_id.clone().map(|s| (b.name.clone(), s))).collect(),
    pinned: bots.iter().filter(|b| b.pinned).map(|b| b.name.clone()).collect(),
  })
}

/// Bring this device in line with `doc`.
pub async fn apply(rt: &Runtime, doc: &Doc) -> Result<()> {
  let pool = &rt.pool;
  let have = store::sections::all(pool).await?;
  for s in &doc.sections {
    match have.iter().find(|h| h.id == s.id) {
      Some(h) if h.name != s.name => store::sections::rename(pool, &s.id, &s.name).await?,
      Some(_) => {}
      None => {
        sqlx_insert(rt, s).await?;
      }
    }
  }
  for h in &have {
    if !doc.sections.iter().any(|s| s.id == h.id) {
      store::sections::delete(pool, &h.id).await?;
    }
  }
  for b in store::bots::list(pool).await? {
    let want = doc.placement.get(&b.name).cloned();
    if b.section_id != want {
      store::bots::set_section(pool, &b.id, want.as_deref()).await?;
    }
    let pin = doc.pinned.contains(&b.name);
    if b.pinned != pin {
      store::bots::pin(pool, &b.id, pin).await?;
    }
  }
  rt.emit(Event::BotsChanged);
  Ok(())
}

async fn sqlx_insert(rt: &Runtime, s: &Section) -> Result<()> {
  store::sections::insert(&rt.pool, &s.id, &s.name, s.position).await
}

/// One sync pass: push local changes, or pull a newer remote file.
pub async fn tick(rt: &Runtime) -> Result<()> {
  let folder = rt.settings().sync_folder;
  if folder.trim().is_empty() {
    return Ok(());
  }
  let dir = PathBuf::from(folder);
  std::fs::create_dir_all(&dir)?;
  let path = dir.join(FILE);
  let mine = local(rt).await?;
  let remote: Option<Doc> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok());
  let last_pushed: i64 = store::state::get(&rt.pool, "sync_pushed").await?.and_then(|v| v.parse().ok()).unwrap_or(0);
  let last_hash = store::state::get(&rt.pool, "sync_hash").await?.unwrap_or_default();
  let hash = fingerprint(&mine);
  match remote {
    // Another device wrote something newer than our last push: take it.
    Some(r) if r.device != mine.device && r.updated > last_pushed && hash == last_hash => {
      apply(rt, &r).await?;
      let applied = local(rt).await?;
      store::state::set(&rt.pool, "sync_hash", &fingerprint(&applied)).await?;
      store::state::set(&rt.pool, "sync_pushed", &r.updated.to_string()).await?;
    }
    // Ours changed since the last pass (or there's no file): write it.
    r if hash != last_hash || r.is_none() => {
      let doc = Doc { updated: store::now(), ..mine };
      let tmp = path.with_extension("json.tmp");
      std::fs::write(&tmp, serde_json::to_string_pretty(&doc)?)?;
      std::fs::rename(tmp, &path)?;
      store::state::set(&rt.pool, "sync_hash", &hash).await?;
      store::state::set(&rt.pool, "sync_pushed", &doc.updated.to_string()).await?;
    }
    _ => {}
  }
  Ok(())
}

fn fingerprint(d: &Doc) -> String {
  let mut d = d.clone();
  d.updated = 0;
  d.device.clear();
  serde_json::to_string(&d).unwrap_or_default()
}
