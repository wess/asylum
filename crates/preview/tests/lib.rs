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
