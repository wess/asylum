use super::*;

#[test]
fn key_names() {
  assert_eq!(key_code("Return"), Some(("Enter", 13)));
  assert_eq!(key_code("esc"), Some(("Escape", 27)));
  assert!(key_code("hyper").is_none());
}

#[test]
fn snapshot_render_lists_elements() {
  let s = Snapshot {
    url: "https://example.com".into(),
    title: "xAI".into(),
    text: "Hello".into(),
    elements: vec!["[1] a \"News\"".into()],
  };
  let r = s.render();
  assert!(r.contains("URL: https://example.com"));
  assert!(r.contains("[1] a \"News\""));
}

// Drives a real Chromium; run with `cargo test -p computer -- --ignored`.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn drives_a_page() {
  let dir = tempfile::tempdir().unwrap();
  let b = Browser::new(dir.path().join("profile"));
  let html = "data:text/html,<title>T</title><input placeholder=q><button onclick=\"document.title='clicked'\">Go</button>";
  let s = b.open("bot", html).await.unwrap();
  assert_eq!(s.elements.len(), 2);
  b.type_text("bot", 1, "hello", false).await.unwrap();
  let s = b.click("bot", 2).await.unwrap();
  assert_eq!(s.title, "clicked");
  assert!(!b.screenshot("bot").await.unwrap().is_empty());
  b.shutdown().await;
}
