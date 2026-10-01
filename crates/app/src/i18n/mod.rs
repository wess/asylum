//! UI translation. Source strings are English and double as keys: `t("New
//! chat")` returns the active language's text or the English itself. Each
//! language is a table in `lang/`. "Follow System" reads the OS preference.

mod lang;

use std::sync::atomic::{AtomicUsize, Ordering};

/// (code, native name, English name)
pub const LANGUAGES: [(&str, &str, &str); 31] = [
  ("en", "English", "English"),
  ("af", "Afrikaans", "Afrikaans"),
  ("ar", "العربية", "Arabic"),
  ("bn", "বাংলা", "Bengali"),
  ("zh-hans", "简体中文", "Chinese (Simplified)"),
  ("zh-hant", "繁體中文", "Chinese (Traditional)"),
  ("cs", "Čeština", "Czech"),
  ("da", "Dansk", "Danish"),
  ("nl", "Nederlands", "Dutch"),
  ("fi", "Suomi", "Finnish"),
  ("fr", "Français", "French"),
  ("de", "Deutsch", "German"),
  ("el", "Ελληνικά", "Greek"),
  ("he", "עברית", "Hebrew"),
  ("hi", "हिन्दी", "Hindi"),
  ("hu", "Magyar", "Hungarian"),
  ("id", "Bahasa Indonesia", "Indonesian"),
  ("it", "Italiano", "Italian"),
  ("ja", "日本語", "Japanese"),
  ("ko", "한국어", "Korean"),
  ("nb", "Norsk bokmål", "Norwegian Bokmål"),
  ("pl", "Polski", "Polish"),
  ("pt", "Português", "Portuguese"),
  ("ru", "Русский", "Russian"),
  ("es", "Español", "Spanish"),
  ("sv", "Svenska", "Swedish"),
  ("th", "ไทย", "Thai"),
  ("tr", "Türkçe", "Turkish"),
  ("uk", "Українська", "Ukrainian"),
  ("ur", "اردو", "Urdu"),
  ("vi", "Tiếng Việt", "Vietnamese"),
];

static ACTIVE: AtomicUsize = AtomicUsize::new(0);

/// The OS's preferred language, mapped onto a supported code.
pub fn system() -> &'static str {
  let pref = std::process::Command::new("defaults")
    .args(["read", "-g", "AppleLanguages"])
    .output()
    .ok()
    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
    .and_then(|s| s.split('"').nth(1).map(str::to_string))
    .or_else(|| std::env::var("LANG").ok())
    .unwrap_or_default();
  matched(&pref)
}

/// Map a locale like "zh-Hant-TW", "pt_BR", or "nb-NO" onto a code.
pub fn matched(locale: &str) -> &'static str {
  let l = locale.to_lowercase().replace('_', "-");
  if l.starts_with("zh") {
    return if l.contains("hant") || l.contains("-tw") || l.contains("-hk") { "zh-hant" } else { "zh-hans" };
  }
  if l.starts_with("no") || l.starts_with("nn") {
    return "nb";
  }
  if l.starts_with("iw") {
    return "he";
  }
  let base = l.split('-').next().unwrap_or("");
  LANGUAGES.iter().find(|(c, _, _)| *c == base).map(|(c, _, _)| *c).unwrap_or("en")
}

/// Activate a language by setting value ("system" or a code).
pub fn set(setting: &str) {
  let code = if setting.is_empty() || setting == "system" { system() } else { matched(setting) };
  let i = LANGUAGES.iter().position(|(c, _, _)| *c == code).unwrap_or(0);
  ACTIVE.store(i, Ordering::Relaxed);
}

pub fn code() -> &'static str {
  LANGUAGES[ACTIVE.load(Ordering::Relaxed)].0
}

pub fn t(en: &'static str) -> &'static str {
  match code() {
    "en" => en,
    c => lang::lookup(c, en).unwrap_or(en),
  }
}

/// `t` with `{}` placeholders filled in order.
pub fn tf(en: &'static str, args: &[&str]) -> String {
  let mut out = t(en).to_string();
  for a in args {
    if let Some(i) = out.find("{}") {
      out.replace_range(i..i + 2, a);
    }
  }
  out
}

#[cfg(test)]
#[path = "../../tests/i18n.rs"]
mod tests;
