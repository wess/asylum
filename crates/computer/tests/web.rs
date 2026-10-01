use super::*;

#[test]
fn parses_duckduckgo_results() {
  let html = r##"<a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fx.ai%2Fnews&amp;rut=1">Grok <b>Bot</b></a>
  <a class="result__snippet" href="#">Always-on &amp; persistent</a>
  <a class="result__a" href="https://docs.x.ai">Docs</a>"##;
  let hits = parse_results(html, 5);
  assert_eq!(hits.len(), 2);
  assert_eq!(hits[0].url, "https://x.ai/news");
  assert_eq!(hits[0].title, "Grok Bot");
  assert_eq!(hits[0].snippet, "Always-on & persistent");
  assert_eq!(hits[1].url, "https://docs.x.ai");
}

#[test]
fn title_and_normalize() {
  assert_eq!(title_of("<html><TITLE lang=en> A &amp; B </TITLE>"), "A & B");
  assert_eq!(normalize("x.ai"), "https://x.ai");
  assert_eq!(normalize("about:blank"), "about:blank");
}

#[test]
fn preview_reads_open_graph() {
  let html = r#"<html><head><title>Fallback</title>
    <meta property="og:title" content="Rust 2.0 &amp; beyond">
    <meta name="description" content="What changed in the new edition.">
    <meta content="Rust Blog" property="og:site_name"></head></html>"#;
  let p = parse_preview(html);
  assert_eq!(p.title, "Rust 2.0 & beyond");
  assert_eq!(p.description, "What changed in the new edition.");
  assert_eq!(p.site, "Rust Blog");
  assert_eq!(parse_preview("<title>Only</title>").title, "Only");
}
