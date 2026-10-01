use super::*;

#[test]
fn resolves_what_was_typed() {
  assert_eq!(resolve("https://example.com/a"), "https://example.com/a");
  assert_eq!(resolve("example.com"), "https://example.com");
  assert_eq!(resolve("docs.rs/serde"), "https://docs.rs/serde");
  assert_eq!(resolve("localhost:3000"), "http://localhost:3000");
  assert_eq!(resolve("rust web frameworks"), "https://duckduckgo.com/?q=rust%20web%20frameworks");
  assert_eq!(resolve("workspace/site/index.html"), "guise://localhost/site/index.html");
  assert_eq!(resolve(""), "about:blank");
}

#[test]
fn workspace_paths_stay_inside() {
  assert_eq!(workspace("../../etc/passwd"), "guise://localhost/etc/passwd");
  assert_eq!(workspace("/reports/q3 notes.html"), "guise://localhost/reports/q3%20notes.html");
  assert_eq!(shown("guise://localhost/reports/q3%20notes.html"), "workspace/reports/q3 notes.html");
  assert_eq!(shown("about:blank"), "");
}
