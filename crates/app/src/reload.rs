//! Settings live reload: bridge the config-file watcher's background
//! callback into the gpui foreground, so saving settings.json re-applies it
//! to the running app. The file is the single source of truth — the Settings
//! surface writes it, and this apply path (also run after each UI write)
//! makes the change visible.

use std::time::Duration;

use futures::StreamExt as _;
use gpui::{App, Context, WindowHandle};

use crate::state::Root;
use crate::{menus, theme};

/// Poll interval for settings.json. Coarse enough to coalesce an editor's
/// multi-write save, fine enough to feel immediate.
const INTERVAL: Duration = Duration::from_millis(250);

/// Seed the freshly created root with the boot-time settings and start
/// watching settings.json; each change re-loads and re-applies it.
pub fn init(window: WindowHandle<Root>, loaded: config::Loaded, cx: &mut App) {
    let (tx, mut rx) = futures::channel::mpsc::unbounded();
    let handle = config::watch(config::default_path(), INTERVAL, move || {
        let _ = tx.unbounded_send(());
    });
    let _ = window.update(cx, |root, _window, cx| {
        root.settings_watch = Some(handle);
        // The boot-time fan-out selection follows the configured default.
        if !loaded.settings.default_agents.is_empty() {
            root.fanout = loaded.settings.default_agents.clone();
        }
        apply(root, loaded, cx);
        root.choose_recommended_agent();
        root.launch_needed = true;
    });
    cx.spawn(async move |cx| {
        while rx.next().await.is_some() {
            if window
                .update(cx, |root, _window, cx| reload(root, cx))
                .is_err()
            {
                break;
            }
        }
    })
    .detach();
}

/// Re-load settings.json and apply the result to the running app.
pub fn reload(root: &mut Root, cx: &mut Context<Root>) {
    apply(root, config::load(&config::default_path()), cx);
}

/// Make `loaded` the app's live configuration: theme, keybindings, menus,
/// input mirrors, and the settings held on [`Root`] for every surface to read.
fn apply(root: &mut Root, loaded: config::Loaded, cx: &mut Context<Root>) {
    let settings = loaded.settings;
    let old_parallel = root.settings.max_parallel_runs;

    if theme::current_name(cx) != settings.theme {
        theme::install(&settings, cx);
    }
    menus::rebind(&settings, cx);
    crate::settings::sync_inputs(root, &settings, cx);

    root.settings = settings;
    if !root.settings.default_agents.is_empty() {
        root.fanout = root.settings.default_agents.clone();
    }
    if old_parallel != root.settings.max_parallel_runs {
        root.launch_needed = true;
    }
    root.settings_diagnostics = loaded.diagnostics;
    if !root.settings_diagnostics.is_empty() {
        root.push_error(
            "Settings need attention",
            format!(
                "{} setting value(s) were ignored. Open Settings for details.",
                root.settings_diagnostics.len()
            ),
        );
    }
    root.refresh_setup();
    cx.refresh_windows();
    cx.notify();
}
