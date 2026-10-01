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

#[test]
fn placeholders() {
  set("en");
  assert_eq!(tf("Create \"{}\" Bot", &["Scout"]), "Create \"Scout\" Bot");
}

#[test]
fn tables_resolve() {
  set("fr");
  assert_ne!(t("Settings"), "Settings");
  set("zh-hans");
  assert_ne!(t("Settings"), "Settings");
  set("ja");
  assert!(tf("Create \"{}\" Bot", &["Scout"]).contains("Scout"));
  assert_ne!(t("Create new Bot"), "Create new Bot");
  set("en");
  assert_eq!(t("Settings"), "Settings");
}
