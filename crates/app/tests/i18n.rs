use super::*;

#[test]
fn matches_locales() {
  assert_eq!(matched("zh-Hant-TW"), "zh-hant");
  assert_eq!(matched("zh_CN"), "zh-hans");
  assert_eq!(matched("pt_BR"), "pt");
  assert_eq!(matched("nb-NO"), "nb");
  assert_eq!(matched("de-DE"), "de");
  assert_eq!(matched("xx"), "en");
}

/// One test: the language is global, so tests that set it can't run in parallel.
#[test]
fn tables_resolve() {
  set("en");
  assert_eq!(tf("Create \"{}\" Agent", &["Scout"]), "Create \"Scout\" Agent");
  set("fr");
  assert_ne!(t("Settings"), "Settings");
  set("zh-hans");
  assert_ne!(t("Settings"), "Settings");
  set("ja");
  assert!(tf("Create \"{}\" Agent", &["Scout"]).contains("Scout"));
  assert_ne!(t("Create new Agent"), "Create new Agent");
  set("en");
  assert_eq!(t("Settings"), "Settings");
}
