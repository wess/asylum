//! Asylum - an Agent Development Environment.
//!
//! `main` loads settings, installs the guise theme, wires the native menu bar,
//! and opens the root window. The heavy lifting lives in the domain crates
//! (`store`, `agent`, `git`, `plugin`) and the view modules here.

mod accounts;
mod browser;
mod control;
mod diff;
mod fleet;
mod icons;
mod integrations;
mod menu;
mod menus;
mod notifications;
mod note;
mod plugins;
mod reload;
mod root;
mod run;
mod search;
#[cfg(feature = "sitecapture")]
mod sitecapture;
mod settings;
mod setup;
mod sidebar;
mod state;
mod theme;
mod workspace;

use gpui::AppContext as _;
use gpui::{point, px, size, App, Bounds, TitlebarOptions, WindowBounds, WindowOptions};

use state::Root;

fn main() {
    // Settings drive the initial theme; a missing file is fine (defaults).
    // Diagnostics are reported when the load is applied (see `reload`).
    let loaded = config::load(&config::default_path());

    #[cfg(feature = "sitecapture")]
    if let Some(path) = std::env::var_os("ASYLUM_SITE_CAPTURE") {
        sitecapture::run(&loaded.settings, path.into()).expect("capture Asylum window");
        return;
    }

    // Launch the mobile companion server on a background thread, serving the
    // same on-disk store the app uses.
    let db_path = state::Root::db_path();
    std::thread::spawn(move || {
        let _ = companion::serve(db_path, "127.0.0.1:8787");
    });

    gpui_platform::application()
        .with_assets(icons::Assets)
        .run(move |cx: &mut App| {
            theme::install(&loaded.settings, cx);

            let bounds = Bounds::centered(None, size(px(1200.0), px(820.0)), cx);
            let root = cx.new(|_cx| Root::seeded());
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(size(px(720.0), px(480.0))),
                        // Transparent native titlebar: our own header draws the
                        // chrome and doubles as the window drag handle, with the
                        // macOS traffic lights floated into the header.
                        titlebar: Some(TitlebarOptions {
                            title: None,
                            appears_transparent: true,
                            traffic_light_position: Some(point(px(14.0), px(16.0))),
                        }),
                        ..Default::default()
                    },
                    {
                        let root = root.clone();
                        move |_window, _cx| root.clone()
                    },
                )
                .expect("open window");

            // The full menu bar, keybindings, and their handlers.
            menus::install(root, window, &loaded.settings, cx);

            // Seed the root with the boot settings and live-reload on change.
            reload::init(window, loaded, cx);

            cx.activate(true);
        });
}
