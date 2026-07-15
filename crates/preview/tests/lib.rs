use super::*;
use std::path::Path;

#[test]
fn classifies_by_extension() {
    assert_eq!(classify(Path::new("README.md")), Kind::Markdown);
    assert_eq!(classify(Path::new("logo.PNG")), Kind::Image);
    assert_eq!(classify(Path::new("spec.pdf")), Kind::Pdf);
    assert_eq!(classify(Path::new("main.rs")), Kind::Text);
    assert_eq!(classify(Path::new("a.out")), Kind::Binary);
    assert_eq!(classify(Path::new("Makefile")), Kind::Text); // no extension
}

#[test]
fn image_mime_types() {
    assert_eq!(image_mime(Path::new("a.png")), "image/png");
    assert_eq!(image_mime(Path::new("a.jpeg")), "image/jpeg");
    assert_eq!(image_mime(Path::new("a.svg")), "image/svg+xml");
}

#[test]
fn renders_markdown_features() {
    let html = render_markdown("# Title\n\n**bold** and `code`\n\n- a\n- b\n");
    assert!(html.contains("<h1>Title</h1>"));
    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("<code>code</code>"));
    assert!(html.contains("<li>a</li>"));

    let table = render_markdown("| a | b |\n|---|---|\n| 1 | 2 |\n");
    assert!(table.contains("<table>"));
}

#[test]
fn renders_callouts_and_mermaid() {
    let callout = render_markdown("> [!warning] Heads up\n> Save your work.\n");
    assert!(callout.contains("callout-warning"));
    assert!(callout.contains("Heads up"));
    // The body stays real Markdown, rendered inside the callout.
    assert!(callout.contains("Save your work."));

    // An untitled callout falls back to a capitalized type label.
    let untitled = render_markdown("> [!tip]\n> Try it.\n");
    assert!(untitled.contains("callout-tip"));
    assert!(untitled.contains("Tip"));

    let diagram = render_markdown("```mermaid\ngraph TD; A-->B;\n```\n");
    assert!(diagram.contains("<pre class=\"mermaid\">"));
    assert!(diagram.contains("A--&gt;B"));

    // Non-mermaid fences are untouched.
    let code = render_markdown("```rust\nfn main() {}\n```\n");
    assert!(code.contains("language-rust") || code.contains("<code"));
    assert!(!code.contains("class=\"mermaid\""));

    // The full document wires in the diagram + highlight scripts.
    let doc = html_markdown("```mermaid\ngraph TD; A-->B;\n```\n");
    assert!(doc.contains("mermaid.esm.min.mjs"));
    assert!(doc.contains("highlight.min.js"));
}

#[test]
fn raw_html_in_markdown_is_rendered_inert() {
    // Script blocks, event handlers, and SVG are escaped to text, never markup.
    let script = render_markdown("<script>alert(1)</script>\n");
    assert!(!script.contains("<script>"), "{script}");
    assert!(script.contains("&lt;script&gt;"), "{script}");

    let handler = render_markdown("<img src=x onerror=alert(1)>\n");
    assert!(!handler.contains("<img"), "{handler}");
    assert!(handler.contains("&lt;img"), "{handler}");

    let svg = render_markdown("<svg onload=alert(1)></svg>\n");
    assert!(!svg.contains("<svg"), "{svg}");

    // Inline raw HTML is escaped too.
    let inline = render_markdown("hi <b onclick=\"x\">there</b>\n");
    assert!(!inline.contains("<b onclick"), "{inline}");
    assert!(inline.contains("&lt;b onclick"), "{inline}");
}

#[test]
fn dangerous_urls_are_dropped_safe_ones_kept() {
    let js = render_markdown("[click](javascript:alert(1))\n");
    assert!(!js.contains("javascript:"), "{js}");

    let data = render_markdown("![x](data:text/html,alert)\n");
    assert!(!data.contains("data:text/html"), "{data}");

    let ok = render_markdown("[home](https://example.com) ![i](img/logo.png)\n");
    assert!(ok.contains("https://example.com"), "{ok}");
    assert!(ok.contains("img/logo.png"), "{ok}");
}

#[test]
fn sanitize_url_scheme_rules() {
    // Safe schemes and relative URLs pass through unchanged.
    assert_eq!(sanitize_url("https://x.com/a"), "https://x.com/a");
    assert_eq!(sanitize_url("http://x"), "http://x");
    assert_eq!(sanitize_url("mailto:a@b.com"), "mailto:a@b.com");
    assert_eq!(sanitize_url("/rel/path"), "/rel/path");
    assert_eq!(sanitize_url("#anchor"), "#anchor");
    assert_eq!(sanitize_url("./a.md#top"), "./a.md#top");
    // Dangerous / unknown schemes are dropped.
    assert_eq!(sanitize_url("javascript:alert(1)"), "");
    assert_eq!(sanitize_url("  JavaScript:alert(1)"), "");
    // Whitespace/control-char obfuscation is normalized before scheme check.
    assert_eq!(sanitize_url("java\tscript:alert(1)"), "");
    assert_eq!(sanitize_url("java\nscript:alert(1)"), "");
    assert_eq!(sanitize_url("data:text/html,x"), "");
    assert_eq!(sanitize_url("vbscript:msgbox"), "");
    assert_eq!(sanitize_url("file:///etc/passwd"), "");
}

#[test]
fn preview_documents_carry_a_csp() {
    let md = html_markdown("# Hi\n");
    assert!(md.contains("Content-Security-Policy"), "{md}");
    assert!(md.contains("default-src 'none'"), "{md}");

    let d = std::env::temp_dir().join(format!("asylum-preview-csp-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&d);
    let txt = d.join("a.txt");
    std::fs::write(&txt, "plain").unwrap();
    let doc = html_document(&txt).unwrap();
    assert!(doc.contains("Content-Security-Policy"), "{doc}");
    assert!(doc.contains("script-src 'none'"), "{doc}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn preview_reads_markdown_and_text() {
    let d = std::env::temp_dir().join(format!("asylum-preview-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&d);
    let md = d.join("doc.md");
    std::fs::write(&md, "# Hi\n").unwrap();
    match preview(&md).unwrap() {
        Preview::Markdown { html } => assert!(html.contains("<h1>Hi</h1>")),
        other => panic!("expected markdown, got {other:?}"),
    }

    let txt = d.join("notes.txt");
    std::fs::write(&txt, "plain text").unwrap();
    match preview(&txt).unwrap() {
        Preview::Text { content } => assert_eq!(content, "plain text"),
        other => panic!("expected text, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn base64_matches_known_vectors() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
}

#[test]
fn html_document_wraps_markdown_and_text() {
    let d = std::env::temp_dir().join(format!("asylum-preview-html-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&d);
    let md = d.join("a.md");
    std::fs::write(&md, "# Hi\n").unwrap();
    let html = html_document(&md).unwrap();
    assert!(html.contains("<!doctype html>"));
    assert!(html.contains("<h1>Hi</h1>"));

    let txt = d.join("a.txt");
    std::fs::write(&txt, "x < y & z").unwrap();
    let html = html_document(&txt).unwrap();
    assert!(html.contains("x &lt; y &amp; z")); // escaped
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn nul_bytes_fall_back_to_binary() {
    let d = std::env::temp_dir().join(format!("asylum-preview-bin-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&d);
    // A .txt file that actually contains NULs is treated as binary.
    let f = d.join("weird.txt");
    std::fs::write(&f, [0u8, 1, 2, 0, 3]).unwrap();
    assert!(matches!(preview(&f).unwrap(), Preview::Binary { .. }));
    let _ = std::fs::remove_dir_all(&d);
}
