use super::*;

#[test]
fn version_compare() {
  assert!(newer("0.2.0", "0.1.9"));
  assert!(newer("1.0.0", "0.10.0"));
  assert!(!newer("0.1.0", "0.1.0"));
}
