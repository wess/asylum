//! Rich file previews.
//!
//! Previews a repository's Markdown, images, PDFs, and docs in place. This
//! crate classifies a path into a [`Kind`] and, for Markdown, renders it to HTML
//! (via `pulldown-cmark`, with GitHub-flavored tables/strikethrough/tasklists).
//! Classification is by extension with a binary-content sniff fallback, so the
//! app can pick the right viewer without opening a huge blob as text.

use std::path::Path;

use pulldown_cmark::{html, Options, Parser};

/// What kind of preview a file warrants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Renderable markdown.
    Markdown,
    /// A raster/vector image (viewer shows it inline).
    Image,
    /// A PDF document.
    Pdf,
    /// Plain/source text (viewer shows it monospaced).
    Text,
    /// Non-text binary with no inline viewer.
    Binary,
}

/// A ready-to-show preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Preview {
    /// Rendered markdown HTML.
    Markdown { html: String },
    /// An image at this path (mime guessed from the extension).
    Image { path: String, mime: String },
    /// A PDF at this path.
    Pdf { path: String },
    /// Plain text contents.
    Text { content: String },
    /// A binary blob; carries its size in bytes.
    Binary { bytes: u64 },
}

/// Classify a path by extension (no I/O).
pub fn classify(path: &Path) -> Kind {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "md" | "markdown" | "mdown" | "mkd" => Kind::Markdown,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "ico" | "avif" => Kind::Image,
        "pdf" => Kind::Pdf,
        "" => Kind::Text,
        _ if is_texty(&ext) => Kind::Text,
        _ => Kind::Binary,
    }
}

/// The guessed MIME type for an image extension.
pub fn image_mime(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Render markdown text to HTML (GFM: tables, strikethrough, task lists).
pub fn render_markdown(source: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(source, opts);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

/// Build a [`Preview`] for `path`, reading it as needed.
pub fn preview(path: &Path) -> std::io::Result<Preview> {
    match classify(path) {
        Kind::Markdown => {
            let src = std::fs::read_to_string(path)?;
            Ok(Preview::Markdown {
                html: render_markdown(&src),
            })
        }
        Kind::Image => Ok(Preview::Image {
            path: path.to_string_lossy().into_owned(),
            mime: image_mime(path),
        }),
        Kind::Pdf => Ok(Preview::Pdf {
            path: path.to_string_lossy().into_owned(),
        }),
        Kind::Text => {
            let bytes = std::fs::read(path)?;
            // A NUL byte in the first chunk means it is really binary.
            if bytes.iter().take(8000).any(|b| *b == 0) {
                Ok(Preview::Binary {
                    bytes: bytes.len() as u64,
                })
            } else {
                Ok(Preview::Text {
                    content: String::from_utf8_lossy(&bytes).into_owned(),
                })
            }
        }
        Kind::Binary => {
            let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            Ok(Preview::Binary { bytes: len })
        }
    }
}

/// Base64-encode `bytes` (standard alphabet, padded). Hand-rolled to keep the
/// crate dependency-light.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// A `data:` URI for an image file (mime guessed from the extension), or `None`
/// if it can't be read. Used to embed images in the preview web view.
pub fn image_data_uri(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(format!("data:{};base64,{}", image_mime(path), base64(&bytes)))
}

/// Render any file as a self-contained HTML preview document: markdown → its
/// rendered HTML, images → an `<img>` (data URI), PDFs → an `<embed>`, text →
/// a `<pre>` block.
pub fn html_document(path: &Path) -> std::io::Result<String> {
    let body = match preview(path)? {
        Preview::Markdown { html } => html,
        Preview::Image { .. } => match image_data_uri(path) {
            Some(uri) => format!("<img src=\"{uri}\" style=\"max-width:100%\">"),
            None => "<p>could not read image</p>".to_string(),
        },
        Preview::Pdf { path } => {
            format!("<embed src=\"file://{path}\" type=\"application/pdf\" width=\"100%\" height=\"100%\">")
        }
        Preview::Text { content } => format!("<pre>{}</pre>", escape_html(&content)),
        Preview::Binary { bytes } => format!("<p>Binary file ({bytes} bytes)</p>"),
    };
    Ok(format!(
        "<!doctype html><meta charset=\"utf-8\"><style>\
         body{{font:14px -apple-system,system-ui,sans-serif;padding:16px;max-width:820px;margin:auto}}\
         pre{{white-space:pre-wrap;font:12px ui-monospace,monospace}}\
         img{{border-radius:6px}}</style>{body}"
    ))
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Extensions we treat as text (source, config, docs). Kept broad; anything
/// unlisted with a NUL byte still falls back to binary at read time.
fn is_texty(ext: &str) -> bool {
    matches!(
        ext,
        "txt" | "rs" | "ts" | "tsx" | "js" | "jsx" | "json" | "toml" | "yaml" | "yml"
            | "html" | "css" | "scss" | "py" | "go" | "rb" | "java" | "c" | "h" | "cpp"
            | "hpp" | "sh" | "bash" | "zsh" | "fish" | "sql" | "xml" | "lock" | "cfg"
            | "ini" | "env" | "gitignore" | "dockerfile" | "make" | "mk" | "swift" | "kt"
            | "php" | "lua" | "vim" | "el" | "clj" | "ex" | "exs" | "erl" | "hs" | "ml"
    )
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
