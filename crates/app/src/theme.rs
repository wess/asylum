//! Install the global guise theme from the app's settings.
//!
//! guise components resolve their colors from one app-global [`guise::Theme`].
//! We pick dark or light from `settings.theme`; the settings live-reload
//! re-installs it whenever the file changes. Toggling writes the flipped
//! value back to settings.json (the file stays the source of truth) and
//! applies it immediately.

use gpui::App;
use guise::Theme;

/// Choose a base theme from a settings token.
pub fn from_name(name: &str) -> Theme {
    match name {
        "light" => Theme::light(),
        _ => Theme::dark(),
    }
}

/// The settings token for the currently installed theme.
pub fn current_name(cx: &App) -> &'static str {
    if cx.global::<Theme>().scheme.is_dark() {
        "dark"
    } else {
        "light"
    }
}

/// Install the theme derived from `settings` as the app global.
pub fn install(settings: &config::Settings, cx: &mut App) {
    from_name(&settings.theme).init(cx);
}

/// Flip between dark and light: apply immediately, refresh every window, and
/// persist the choice to settings.json (best effort — the live reload of the
/// same value is a no-op).
pub fn toggle(cx: &mut App) {
    let next = cx.global::<Theme>().scheme.toggled();
    cx.global_mut::<Theme>().scheme = next;
    cx.refresh_windows();
    let name = if next.is_dark() { "dark" } else { "light" };
    if let Err(e) = config::edit::set_key(&config::default_path(), "theme", &format!("{name:?}")) {
        eprintln!("settings: {e}");
    }
}
