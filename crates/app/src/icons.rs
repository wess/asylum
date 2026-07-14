//! Lucide icons.
//!
//! guise's built-in [`Icon`](guise::Icon) draws Unicode glyphs; for crisp,
//! consistent line icons we render [Lucide](https://lucide.dev) SVGs through
//! gpui's `svg()` element. The SVGs are embedded in the binary (an
//! [`AssetSource`]) and use `currentColor`, so [`icon`] tints them with a
//! `text_color`.

use std::borrow::Cow;

use gpui::prelude::*;
use gpui::{px, svg, AssetSource, SharedString, Svg};

/// The embedded Lucide icon set, wired via `Application::with_assets`.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        let name = path.strip_prefix("icons/").and_then(|n| n.strip_suffix(".svg"));
        Ok(name.and_then(bytes).map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

/// A Lucide `svg()` element for `name`, sized to `size` px. Set `.text_color(..)`
/// on the result to tint it.
pub fn icon(name: &str, size: f32) -> Svg {
    svg().path(format!("icons/{name}.svg")).size(px(size)).flex_none()
}

/// Resolve an icon name to its embedded SVG bytes.
fn bytes(name: &str) -> Option<&'static [u8]> {
    macro_rules! set {
        ($($n:literal),* $(,)?) => {
            match name {
                $($n => Some(include_bytes!(concat!("../../../assets/icons/", $n, ".svg")) as &[u8]),)*
                _ => None,
            }
        };
    }
    set!(
        "list-todo", "git-compare", "search", "github", "terminal", "file-pen", "eye", "globe",
        "puzzle", "circle-user", "inbox", "chevron-right", "chevron-down", "folder", "git-branch",
        "play", "circle", "circle-check", "circle-x", "loader", "star", "sun", "command", "plus",
        "settings"
    )
}
