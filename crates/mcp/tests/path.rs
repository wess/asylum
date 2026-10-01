use super::*;

#[test]
fn merges_without_repeats() {
  let p = merge("/opt/homebrew/bin:/usr/bin", "/usr/bin:/bin", &["/Users/x/.asdf/shims".into()]);
  assert_eq!(p, "/opt/homebrew/bin:/usr/bin:/bin:/Users/x/.asdf/shims:/usr/local/bin:/usr/sbin");
}

#[test]
fn login_path_finds_common_dirs() {
  assert!(login().contains("/usr/bin"));
}
