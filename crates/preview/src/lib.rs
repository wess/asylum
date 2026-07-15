//! Rich file previews.
//!
//! Previews a repository's Markdown, images, PDFs, and docs in place. This
//! crate classifies a path into a [`Kind`] and, for Markdown, renders it to HTML
//! (via `pulldown-cmark`, with GitHub-flavored tables/strikethrough/tasklists).
//! Classification is by extension with a binary-content sniff fallback, so the
//! app can pick the right viewer without opening a huge blob as text.

use std::path::Path;

use pulldown_cmark::{html, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

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

/// Render markdown text to sanitized HTML (GFM: tables, strikethrough, task
/// lists), plus callout blockquotes (`> [!type]`) and Mermaid code fences.
///
/// Repository Markdown is untrusted: raw HTML in the source is **escaped to inert
/// text** (so `<script>`, `<img onerror>`, `<svg onload>` cannot execute), and
/// link/image URLs are restricted to safe schemes (`javascript:`, `data:`,
/// `file:`, … are dropped). Callouts and Mermaid blocks are the only structural
/// HTML, and they are generated here from trusted templates, not passed through
/// from the source. See also the CSP applied by [`html_markdown`].
pub fn render_markdown(source: &str) -> String {
    let mut out = String::new();
    for segment in segment_callouts(source) {
        match segment {
            Segment::Plain(md) => out.push_str(&render_body(&md)),
            Segment::Callout {
                class,
                icon,
                heading,
                body,
            } => {
                out.push_str(&format!(
                    "<div class=\"callout {class}\">\n<div class=\"callout-title\">{icon} {}</div>\n",
                    escape_html(&heading)
                ));
                out.push_str(&render_body(&body));
                out.push_str("\n</div>\n");
            }
        }
    }
    out
}

/// Render one span of Markdown to HTML with raw HTML escaped and URLs
/// sanitized. Mermaid fences become trusted `<pre class="mermaid">` blocks.
fn render_body(source: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);

    let mut events = Vec::new();
    let mut mermaid: Option<String> = None;
    for event in Parser::new_ext(source, opts) {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
                if lang.as_ref().eq_ignore_ascii_case("mermaid") =>
            {
                mermaid = Some(String::new());
            }
            Event::Text(text) if mermaid.is_some() => {
                mermaid.as_mut().expect("mermaid buffer").push_str(&text);
            }
            Event::End(TagEnd::CodeBlock) if mermaid.is_some() => {
                let code = mermaid.take().unwrap_or_default();
                let html = format!("<pre class=\"mermaid\">{}</pre>", escape_html(&code));
                events.push(Event::Html(html.into()));
            }
            // Untrusted raw HTML from the source is rendered as inert text.
            Event::Html(s) => events.push(Event::Text(s)),
            Event::InlineHtml(s) => events.push(Event::Text(s)),
            // Restrict link/image destinations to safe schemes.
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => events.push(Event::Start(Tag::Link {
                link_type,
                dest_url: sanitize_url(&dest_url).into(),
                title,
                id,
            })),
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => events.push(Event::Start(Tag::Image {
                link_type,
                dest_url: sanitize_url(&dest_url).into(),
                title,
                id,
            })),
            other => events.push(other),
        }
    }
    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    out
}

/// A span of the source: plain Markdown or a callout with a Markdown body.
enum Segment {
    Plain(String),
    Callout {
        class: &'static str,
        icon: &'static str,
        heading: String,
        body: String,
    },
}

/// Split the source into plain spans and callout blocks (`> [!type] Title`).
/// The callout body stays as Markdown so it is rendered (and sanitized) by
/// [`render_body`] rather than injected as raw HTML.
fn segment_callouts(source: &str) -> Vec<Segment> {
    let lines: Vec<&str> = source.lines().collect();
    let mut segments = Vec::new();
    let mut plain = String::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((kind, title)) = callout_header(lines[i]) else {
            plain.push_str(lines[i]);
            plain.push('\n');
            i += 1;
            continue;
        };
        if !plain.is_empty() {
            segments.push(Segment::Plain(std::mem::take(&mut plain)));
        }
        i += 1;
        let mut body = Vec::new();
        while i < lines.len() && lines[i].trim_start().starts_with('>') {
            body.push(strip_quote(lines[i]));
            i += 1;
        }
        let (class, icon) = callout_style(&kind);
        let heading = if title.is_empty() {
            capitalize(&kind)
        } else {
            title
        };
        segments.push(Segment::Callout {
            class,
            icon,
            heading,
            body: body.join("\n"),
        });
    }
    if !plain.is_empty() {
        segments.push(Segment::Plain(plain));
    }
    segments
}

/// Uppercase the first character of `s`.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect::<String>())
        .unwrap_or_default()
}

/// Restrict a link/image URL to a safe scheme. Relative URLs and anchors pass
/// through; `http`, `https`, and `mailto` pass through; everything else
/// (`javascript:`, `data:`, `vbscript:`, `file:`, custom schemes) is dropped to
/// an empty string. Whitespace and control characters - a common scheme
/// obfuscation (`java\tscript:`) that browsers ignore - are stripped before the
/// scheme is read.
fn sanitize_url(url: &str) -> String {
    let normalized: String = url
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && !c.is_control())
        .collect();
    match uri_scheme(&normalized) {
        Some(scheme) if matches!(scheme.as_str(), "http" | "https" | "mailto") => url.to_string(),
        Some(_) => String::new(),
        None => url.to_string(),
    }
}

/// The URI scheme of `url` (RFC 3986: `ALPHA *(ALPHA / DIGIT / "+" / "-" / ".")`
/// then `:`), lowercased, or `None` when there is no scheme (a relative URL).
fn uri_scheme(url: &str) -> Option<String> {
    let (head, _) = url.split_once(':')?;
    let mut chars = head.chars();
    if !chars.next()?.is_ascii_alphabetic() {
        return None;
    }
    head.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        .then(|| head.to_ascii_lowercase())
}

/// Parse a `> [!type] Title` callout header into its lowercase type and title.
fn callout_header(line: &str) -> Option<(String, String)> {
    let rest = line.trim_start().strip_prefix('>')?.trim_start();
    let rest = rest.strip_prefix("[!")?;
    let close = rest.find(']')?;
    let kind = rest[..close].trim().to_lowercase();
    if kind.is_empty() {
        return None;
    }
    Some((kind, rest[close + 1..].trim().to_string()))
}

/// Strip one blockquote marker (`>` and an optional space) from a line.
fn strip_quote(line: &str) -> String {
    let rest = line.trim_start();
    let rest = rest.strip_prefix('>').unwrap_or(rest);
    rest.strip_prefix(' ').unwrap_or(rest).to_string()
}

/// Map a callout type to its CSS class and leading icon.
fn callout_style(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "info" | "todo" => ("callout-info", "ℹ️"),
        "tip" | "hint" | "important" => ("callout-tip", "💡"),
        "success" | "check" | "done" => ("callout-success", "✅"),
        "question" | "help" | "faq" => ("callout-question", "❓"),
        "warning" | "caution" | "attention" => ("callout-warning", "⚠️"),
        "failure" | "fail" | "missing" | "danger" | "error" | "bug" => ("callout-danger", "🔴"),
        "example" => ("callout-example", "🧩"),
        "quote" | "cite" => ("callout-quote", "❝"),
        _ => ("callout-note", "📝"),
    }
}

/// Render Markdown into the self-contained document used by native web views.
/// Includes callout styling, Mermaid diagram rendering, and code syntax
/// highlighting; the diagram/highlight scripts degrade gracefully to plain
/// text when the view is offline.
pub fn html_markdown(source: &str) -> String {
    let body = render_markdown(source);
    format!("<!doctype html><meta charset=\"utf-8\">{MARKDOWN_CSP}{MARKDOWN_HEAD}{body}{MARKDOWN_SCRIPTS}")
}

/// Content-Security-Policy for rendered-Markdown documents. `default-src 'none'`
/// denies everything by default; the highlight/diagram helpers are pinned to the
/// jsdelivr CDN, images may be local/data/https, and only the document's own
/// (trusted, generated) inline styles/scripts run. User Markdown contributes no
/// script - it is escaped by [`render_markdown`] - so nothing user-authored can
/// execute even before this policy applies.
const MARKDOWN_CSP: &str = "<meta http-equiv=\"Content-Security-Policy\" content=\"\
    default-src 'none'; \
    img-src 'self' data: file: https:; \
    style-src 'unsafe-inline' https://cdn.jsdelivr.net; \
    script-src 'unsafe-inline' https://cdn.jsdelivr.net; \
    font-src data: https://cdn.jsdelivr.net; \
    connect-src https://cdn.jsdelivr.net; \
    base-uri 'none'; frame-ancestors 'none'; object-src 'none'\">";

/// Styles shared by every rendered-Markdown document.
const MARKDOWN_HEAD: &str = "<style>\
    :root{color-scheme:light dark}\
    body{font:14px -apple-system,system-ui,sans-serif;padding:20px;max-width:760px;margin:auto;line-height:1.55}\
    h1,h2,h3{line-height:1.25}\
    pre,code{font:12px ui-monospace,monospace}\
    pre{padding:12px;overflow:auto;border:1px solid color-mix(in srgb,currentColor 18%,transparent);border-radius:6px}\
    pre.mermaid{border:none;padding:0;text-align:center;background:none}\
    blockquote{margin-left:0;padding-left:12px;border-left:3px solid #3b82f6}\
    a{color:#2563eb} table{border-collapse:collapse} td,th{padding:5px 8px;border:1px solid #8886}\
    img{max-width:100%;border-radius:6px}\
    .callout{margin:14px 0;border:1px solid color-mix(in srgb,var(--cc) 30%,transparent);border-left:4px solid var(--cc);border-radius:8px;padding:10px 14px;background:color-mix(in srgb,var(--cc) 8%,transparent)}\
    .callout-title{font-weight:600;margin-bottom:2px}\
    .callout>p{margin:4px 0}\
    .callout-note{--cc:#7c8792}\
    .callout-info{--cc:#3b82f6}.callout-tip{--cc:#10b981}.callout-success{--cc:#22c55e}\
    .callout-question{--cc:#a855f7}.callout-warning{--cc:#f59e0b}.callout-danger{--cc:#ef4444}\
    .callout-example{--cc:#6366f1}.callout-quote{--cc:#9ca3af}\
    </style>";

/// Diagram + highlight scripts appended after the body. Loaded from a CDN and
/// wrapped so a missing network leaves the source text visible.
const MARKDOWN_SCRIPTS: &str = "<link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/gh/highlightjs/cdn-release/build/styles/github-dark.min.css\">\
    <script>\
    (function(){\
      var h=document.createElement('script');\
      h.src='https://cdn.jsdelivr.net/gh/highlightjs/cdn-release/build/highlight.min.js';\
      h.onload=function(){document.querySelectorAll('pre code').forEach(function(b){window.hljs&&window.hljs.highlightElement(b);});};\
      document.head.appendChild(h);\
    })();\
    </script>\
    <script type=\"module\">\
      try{\
        var m=await import('https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs');\
        var dark=matchMedia('(prefers-color-scheme: dark)').matches;\
        m.default.initialize({startOnLoad:true,theme:dark?'dark':'default'});\
      }catch(e){}\
    </script>";

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
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
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
    Some(format!(
        "data:{};base64,{}",
        image_mime(path),
        base64(&bytes)
    ))
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
            // The path is a real local file; escape it so it cannot break out of
            // the attribute.
            format!(
                "<embed src=\"file://{}\" type=\"application/pdf\" width=\"100%\" height=\"100%\">",
                escape_attr(&path)
            )
        }
        Preview::Text { content } => format!("<pre>{}</pre>", escape_html(&content)),
        Preview::Binary { bytes } => format!("<p>Binary file ({bytes} bytes)</p>"),
    };
    Ok(format!(
        "<!doctype html><meta charset=\"utf-8\">{DOCUMENT_CSP}<style>\
         body{{font:14px -apple-system,system-ui,sans-serif;padding:16px;max-width:820px;margin:auto}}\
         pre{{white-space:pre-wrap;font:12px ui-monospace,monospace}}\
         img{{border-radius:6px}}</style>{body}"
    ))
}

/// CSP for generic file-preview documents: no scripts at all, local/data/https
/// images, and a local PDF embed via `object-src`.
const DOCUMENT_CSP: &str = "<meta http-equiv=\"Content-Security-Policy\" content=\"\
    default-src 'none'; \
    img-src 'self' data: file: https:; \
    style-src 'unsafe-inline'; \
    object-src 'self' file:; \
    script-src 'none'; base-uri 'none'; frame-ancestors 'none'\">";

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Like [`escape_html`] but also escapes the double quote, for use inside a
/// double-quoted HTML attribute value.
fn escape_attr(s: &str) -> String {
    escape_html(s).replace('"', "&quot;")
}

/// Extensions we treat as text (source, config, docs). Kept broad; anything
/// unlisted with a NUL byte still falls back to binary at read time.
fn is_texty(ext: &str) -> bool {
    matches!(
        ext,
        "txt"
            | "rs"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "json"
            | "toml"
            | "yaml"
            | "yml"
            | "html"
            | "css"
            | "scss"
            | "py"
            | "go"
            | "rb"
            | "java"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "sh"
            | "bash"
            | "zsh"
            | "fish"
            | "sql"
            | "xml"
            | "lock"
            | "cfg"
            | "ini"
            | "env"
            | "gitignore"
            | "dockerfile"
            | "make"
            | "mk"
            | "swift"
            | "kt"
            | "php"
            | "lua"
            | "vim"
            | "el"
            | "clj"
            | "ex"
            | "exs"
            | "erl"
            | "hs"
            | "ml"
    )
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
