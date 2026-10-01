use super::*;

#[test]
fn reads_resource_metadata() {
  let h = r#"Bearer realm="mcp", resource_metadata="https://mcp.x.com/.well-known/oauth-protected-resource""#;
  assert_eq!(resource_metadata(h).unwrap(), "https://mcp.x.com/.well-known/oauth-protected-resource");
  assert!(resource_metadata("Bearer").is_none());
}
