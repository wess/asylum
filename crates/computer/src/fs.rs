//! The Bot's filesystem, confined to its home: every path the model supplies
//! is resolved against home and rejected if it escapes.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::{Component, Path, PathBuf};

pub fn resolve(home: &Path, input: &str) -> Result<PathBuf> {
  let input = input.trim();
  let rel = input
    .strip_prefix("~/")
    .or_else(|| (input == "~").then_some(""))
    .unwrap_or(input);
  let candidate = Path::new(rel);
  let joined = if candidate.is_absolute() {
    match candidate.strip_prefix(home) {
      Ok(inner) => home.join(inner),
      Err(_) => bail!("{input} is outside this Bot's computer"),
    }
  } else {
    home.join(candidate)
  };
  let mut out = PathBuf::new();
  for c in joined.components() {
    match c {
      Component::ParentDir => {
        out.pop();
      }
      Component::CurDir => {}
      other => out.push(other),
    }
  }
  if !out.starts_with(home) {
    bail!("{input} is outside this Bot's computer");
  }
  Ok(out)
}

/// Display a resolved path relative to home.
pub fn show(home: &Path, path: &Path) -> String {
  match path.strip_prefix(home) {
    Ok(p) if p.as_os_str().is_empty() => "~".into(),
    Ok(p) => format!("~/{}", p.display()),
    Err(_) => path.display().to_string(),
  }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Entry {
  pub name: String,
  pub path: String,
  pub dir: bool,
  pub size: u64,
  pub modified: i64,
}

pub fn list(home: &Path, dir: &str) -> Result<Vec<Entry>> {
  let path = resolve(home, dir)?;
  let mut out = Vec::new();
  for entry in std::fs::read_dir(&path).with_context(|| format!("cannot list {dir}"))? {
    let entry = entry?;
    let meta = entry.metadata()?;
    let modified = meta
      .modified()
      .ok()
      .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
      .map(|d| d.as_millis() as i64)
      .unwrap_or(0);
    out.push(Entry {
      name: entry.file_name().to_string_lossy().into_owned(),
      path: show(home, &entry.path()),
      dir: meta.is_dir(),
      size: meta.len(),
      modified,
    });
  }
  out.sort_by(|a, b| b.dir.cmp(&a.dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
  Ok(out)
}

pub fn read(home: &Path, file: &str) -> Result<String> {
  let path = resolve(home, file)?;
  let bytes = std::fs::read(&path).with_context(|| format!("cannot read {file}"))?;
  match String::from_utf8(bytes) {
    Ok(s) => Ok(s),
    Err(e) => Ok(format!("[binary file, {} bytes]", e.as_bytes().len())),
  }
}

pub fn write(home: &Path, file: &str, content: &str) -> Result<String> {
  let path = resolve(home, file)?;
  if let Some(dir) = path.parent() {
    std::fs::create_dir_all(dir)?;
  }
  std::fs::write(&path, content)?;
  Ok(show(home, &path))
}

pub fn write_bytes(home: &Path, file: &str, content: &[u8]) -> Result<String> {
  let path = resolve(home, file)?;
  if let Some(dir) = path.parent() {
    std::fs::create_dir_all(dir)?;
  }
  std::fs::write(&path, content)?;
  Ok(show(home, &path))
}

/// Replace exactly one occurrence of `old` with `new`.
pub fn edit(home: &Path, file: &str, old: &str, new: &str) -> Result<()> {
  let path = resolve(home, file)?;
  let text = std::fs::read_to_string(&path).with_context(|| format!("cannot read {file}"))?;
  match text.matches(old).count() {
    0 => bail!("text not found in {file}"),
    1 => Ok(std::fs::write(&path, text.replacen(old, new, 1))?),
    n => bail!("text appears {n} times in {file}; include more context"),
  }
}

pub fn remove(home: &Path, target: &str) -> Result<()> {
  let path = resolve(home, target)?;
  if path == home {
    bail!("refusing to delete the home directory");
  }
  if path.is_dir() {
    std::fs::remove_dir_all(&path)?;
  } else {
    std::fs::remove_file(&path)?;
  }
  Ok(())
}

pub fn rename(home: &Path, from: &str, to: &str) -> Result<()> {
  let a = resolve(home, from)?;
  let b = resolve(home, to)?;
  if let Some(dir) = b.parent() {
    std::fs::create_dir_all(dir)?;
  }
  std::fs::rename(a, b)?;
  Ok(())
}

pub fn mkdir(home: &Path, dir: &str) -> Result<()> {
  Ok(std::fs::create_dir_all(resolve(home, dir)?)?)
}

/// Files under `dir` whose name or contents contain `needle` (case-insensitive).
pub fn search(home: &Path, dir: &str, needle: &str, limit: usize) -> Result<Vec<String>> {
  let root = resolve(home, dir)?;
  let needle = needle.to_lowercase();
  let mut out = Vec::new();
  let mut stack = vec![root];
  while let Some(d) = stack.pop() {
    let Ok(entries) = std::fs::read_dir(&d) else { continue };
    for e in entries.flatten() {
      let p = e.path();
      if p.is_dir() {
        stack.push(p);
        continue;
      }
      let name_hit = e.file_name().to_string_lossy().to_lowercase().contains(&needle);
      let line_hit = (!name_hit)
        .then(|| std::fs::read_to_string(&p).ok())
        .flatten()
        .and_then(|text| {
          text
            .lines()
            .enumerate()
            .find(|(_, l)| l.to_lowercase().contains(&needle))
            .map(|(i, l)| format!("{}:{}: {}", show(home, &p), i + 1, l.trim()))
        });
      if name_hit {
        out.push(show(home, &p));
      } else if let Some(hit) = line_hit {
        out.push(hit);
      }
      if out.len() >= limit {
        return Ok(out);
      }
    }
  }
  Ok(out)
}

#[cfg(test)]
#[path = "../tests/fs.rs"]
mod tests;
