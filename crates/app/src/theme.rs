//! Install the global guise theme from the app's settings.
//!
//! guise components resolve their colors from one app-global [`guise::Theme`].
//! We pick dark or light from `settings.theme` and install it once at startup;
//! toggling re-installs and refreshes the windows (see [`toggle`]).

use gpui::App;
use guise::Theme;

/// Choose a base theme from a settings token.
pub fn from_name(name: &str) -> Theme {
    match name {
        "light" => Theme::light(),
        _ => Theme::dark(),
    }
}

/// Install the theme derived from `settings` as the app global.
pub fn install(settings: &config::Settings, cx: &mut App) {
    from_name(&settings.theme).init(cx);
}

/// Flip between dark and light and refresh every window.
pub fn toggle(cx: &mut App) {
    let next = cx.global::<Theme>().scheme.toggled();
    cx.global_mut::<Theme>().scheme = next;
    cx.refresh_windows();
}
