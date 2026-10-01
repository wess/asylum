//! Attachments: at most six per message; documents, images, and audio up to
//! 25 MB, video up to 200 MB. Accepted files are copied into the workspace.

use crate::part::Part;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub const MAX_FILES: usize = 6;
pub const MAX_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_VIDEO: u64 = 200 * 1024 * 1024;

const VIDEO: [&str; 5] = ["mp4", "mov", "m4v", "webm", "avi"];
const OK: [&str; 49] = [
  "png", "jpg", "jpeg", "gif", "webp", "heic", "svg", "mp3", "wav", "m4a", "aac", "ogg", "flac", "aiff", "pdf", "txt",
  "md", "markdown", "rtf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "csv", "tsv", "json", "yaml", "yml", "xml",
  "html", "htm", "eml", "ipynb", "rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "c", "h", "cpp", "rb", "sh", "sql",
];

pub fn check(path: &Path) -> Result<u64> {
  let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
  let meta = match std::fs::metadata(path) {
    Ok(m) if m.is_file() => m,
    _ => bail!("{name}: the upload isn't finished or the file is missing"),
  };
  let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
  let video = VIDEO.contains(&ext.as_str());
  if !video && !OK.contains(&ext.as_str()) && !is_text(path) {
    bail!("{name}: this file type isn't supported");
  }
  let limit = if video { MAX_VIDEO } else { MAX_BYTES };
  if meta.len() > limit {
    bail!("{name}: too large (limit {} MB)", limit / 1024 / 1024);
  }
  if meta.len() == 0 {
    bail!("{name}: the file is empty or damaged");
  }
  if ext == "pdf" && encrypted_pdf(path) {
    bail!("{name}: password-protected files can't be read");
  }
  Ok(meta.len())
}

fn is_text(path: &Path) -> bool {
  std::fs::read(path)
    .map(|b| {
      let head = &b[..b.len().min(4096)];
      std::str::from_utf8(head).is_ok() && !head.contains(&0)
    })
    .unwrap_or(false)
}

fn encrypted_pdf(path: &Path) -> bool {
  std::fs::read(path).map(|b| b.windows(8).any(|w| w == b"/Encrypt")).unwrap_or(false)
}

/// Validate and copy every file, or fail on the first problem.
pub fn ingest(workspace: &Path, files: &[PathBuf]) -> Result<Vec<Part>> {
  if files.len() > MAX_FILES {
    bail!("You can attach up to {MAX_FILES} files at once.");
  }
  let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
  let mut out = Vec::new();
  for f in files {
    let size = check(f)?;
    let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
    let rel = unique(workspace, &format!("uploads/{day}/{name}"));
    let dest = workspace.join(&rel);
    if let Some(d) = dest.parent() {
      std::fs::create_dir_all(d)?;
    }
    std::fs::copy(f, &dest)?;
    out.push(Part::Attachment { mime: crate::tools::computer::mime(&name), name, path: dest.display().to_string(), size });
  }
  Ok(out)
}

fn unique(ws: &Path, rel: &str) -> String {
  if !ws.join(rel).exists() {
    return rel.to_string();
  }
  let (stem, ext) = match rel.rsplit_once('.') {
    Some((s, e)) => (s.to_string(), format!(".{e}")),
    None => (rel.to_string(), String::new()),
  };
  (2..).map(|n| format!("{stem} {n}{ext}")).find(|c| !ws.join(c).exists()).unwrap_or_default()
}

#[cfg(test)]
#[path = "../../tests/attach.rs"]
mod tests;
