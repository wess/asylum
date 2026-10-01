use super::*;

#[test]
fn span_has_ids_names_and_no_content() {
  let b = body("tool", &[("tool", "browser_open"), ("outcome", "done")], 250, 2_000_000_000);
  let span = &b["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
  assert_eq!(span["name"], "tool");
  assert_eq!(span["traceId"].as_str().unwrap().len(), 32);
  assert_eq!(span["spanId"].as_str().unwrap().len(), 16);
  assert_eq!(span["startTimeUnixNano"], "1750000000");
  let attrs = span["attributes"].as_array().unwrap();
  assert!(attrs.iter().any(|a| a["key"] == "asylum.tool" && a["value"]["stringValue"] == "browser_open"));
  assert!(attrs.iter().any(|a| a["key"] == "asylum.surface"));
}
