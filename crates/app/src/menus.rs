//! The native macOS menu bar and its actions.
//!
//! Menu items dispatch gpui [`Action`]s; the same actions are bound to keyboard
//! shortcuts (so the menu shows the shortcut *and* the key works), and handled
//! globally against the root window. A standard macOS menu bar adapted to the
//! ADE's surfaces (File/View for tasks/terminals/tabs).
//!
//! Shortcuts come from the [`config::Keymap`]: the compiled defaults layered
//! under the user's `keybindings` in settings.json, re-applied on every
//! settings reload (see [`rebind`]).

use gpui::{actions, App, Entity, KeyBinding, Keystroke, Menu, MenuItem, OsAction, WindowHandle};

use crate::logs;
use crate::root::open_project;
use crate::state::{Root, View};
use crate::theme;

actions!(
    asylum,
    [
        // App
        About,
        OpenSettings,
        OpenSettingsFile,
        Quit,
        // File
        NewTask,
        Open,
        NewTerminal,
        SplitRight,
        CloseTab,
        // Edit (native, via os_action)
        Undo,
        Redo,
        Cut,
        Copy,
        Paste,
        SelectAll,
        FindInProject,
        // View
        CommandPalette,
        QuickOpen,
        ToggleTheme,
        GoTasks,
        GoDiff,
        GoSearch,
        GoIntegrations,
        GoTerminal,
        GoEditor,
        GoBrowser,
        GoPreview,
        GoPlugins,
        GoAccounts,
        GoInbox,
        // Task
        RunFanout,
        // Help
        Documentation,
        OpenLogFolder,
    ]
);

/// Install the menu bar, keybindings, and action handlers.
pub fn install(
    root: Entity<Root>,
    window: WindowHandle<Root>,
    settings: &config::Settings,
    cx: &mut App,
) {
    register(root, window, cx);
    rebind(settings, cx);
}

/// (Re-)apply the keymap from `settings` and refresh the menu bar so its
/// shortcut hints match. Called at boot and on every settings reload.
pub fn rebind(settings: &config::Settings, cx: &mut App) {
    cx.clear_key_bindings();
    cx.bind_keys(bindings(settings));
    cx.set_menus(menus());
}

fn menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "Asylum".into(),
            disabled: false,
            items: vec![
                MenuItem::action("About Asylum", About),
                MenuItem::separator(),
                MenuItem::action("Settings…", OpenSettings),
                MenuItem::action("Open settings.json", OpenSettingsFile),
                MenuItem::separator(),
                MenuItem::action("Quit Asylum", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            disabled: false,
            items: vec![
                MenuItem::action("New Task", NewTask),
                MenuItem::action("Open…", Open),
                MenuItem::separator(),
                MenuItem::action("New Terminal", NewTerminal),
                MenuItem::action("Split Right", SplitRight),
                MenuItem::separator(),
                MenuItem::action("Close Tab", CloseTab),
            ],
        },
        Menu {
            name: "Edit".into(),
            disabled: false,
            items: vec![
                MenuItem::os_action("Undo", Undo, OsAction::Undo),
                MenuItem::os_action("Redo", Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", Cut, OsAction::Cut),
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
                MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
                MenuItem::separator(),
                MenuItem::action("Find in Project", FindInProject),
            ],
        },
        Menu {
            name: "View".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Command Palette", CommandPalette),
                MenuItem::action("Quick Open", QuickOpen),
                MenuItem::separator(),
                MenuItem::action("Tasks", GoTasks),
                MenuItem::action("Review", GoDiff),
                MenuItem::action("Search", GoSearch),
                MenuItem::action("Integrations", GoIntegrations),
                MenuItem::action("Terminal", GoTerminal),
                MenuItem::action("Editor", GoEditor),
                MenuItem::action("Browser", GoBrowser),
                MenuItem::action("Preview", GoPreview),
                MenuItem::action("Plugins", GoPlugins),
                MenuItem::action("Accounts", GoAccounts),
                MenuItem::action("Inbox", GoInbox),
                MenuItem::separator(),
                MenuItem::action("Toggle Theme", ToggleTheme),
            ],
        },
        Menu {
            name: "Help".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Asylum Documentation", Documentation),
                MenuItem::action("Open Log Folder", OpenLogFolder),
            ],
        },
    ]
}

/// Build the gpui keybindings from the resolved keymap. A typo in
/// settings.json (bad chord, unknown action) skips that entry with a note —
/// it must never take the rest of the keymap down.
fn bindings(settings: &config::Settings) -> Vec<KeyBinding> {
    let keymap = config::Keymap::from_settings(&settings.keybindings);
    let mut out = Vec::new();
    for (chord, action) in keymap.bindings() {
        if chord
            .split_whitespace()
            .any(|part| Keystroke::parse(part).is_err())
        {
            tracing::warn!(chord, "settings: keybindings: cannot parse chord");
            continue;
        }
        match binding(chord, action) {
            Some(b) => out.push(b),
            None => tracing::warn!(action, "settings: keybindings: unknown action"),
        }
    }
    out
}

/// One keymap entry as a gpui binding. The action names here are the
/// vocabulary settings.json `keybindings` entries can use.
fn binding(chord: &str, action: &str) -> Option<KeyBinding> {
    Some(match action {
        "command_palette" => KeyBinding::new(chord, CommandPalette, None),
        "quick_open" => KeyBinding::new(chord, QuickOpen, None),
        "find_in_project" => KeyBinding::new(chord, FindInProject, None),
        "new_task" => KeyBinding::new(chord, NewTask, None),
        "open_project" => KeyBinding::new(chord, Open, None),
        "run_fanout" => KeyBinding::new(chord, RunFanout, None),
        "review_diff" => KeyBinding::new(chord, GoDiff, None),
        "new_terminal" => KeyBinding::new(chord, NewTerminal, None),
        "split_right" => KeyBinding::new(chord, SplitRight, None),
        "close_tab" => KeyBinding::new(chord, CloseTab, None),
        "settings" => KeyBinding::new(chord, OpenSettings, None),
        "open_settings_file" => KeyBinding::new(chord, OpenSettingsFile, None),
        "toggle_theme" => KeyBinding::new(chord, ToggleTheme, None),
        "switch_account" => KeyBinding::new(chord, GoAccounts, None),
        "notifications" => KeyBinding::new(chord, GoInbox, None),
        "quit" => KeyBinding::new(chord, Quit, None),
        "tasks" => KeyBinding::new(chord, GoTasks, None),
        "search" => KeyBinding::new(chord, GoSearch, None),
        "integrations" => KeyBinding::new(chord, GoIntegrations, None),
        "terminal" => KeyBinding::new(chord, GoTerminal, None),
        "editor" => KeyBinding::new(chord, GoEditor, None),
        "browser" => KeyBinding::new(chord, GoBrowser, None),
        "preview" => KeyBinding::new(chord, GoPreview, None),
        "plugins" => KeyBinding::new(chord, GoPlugins, None),
        _ => return None,
    })
}

fn register(root: Entity<Root>, window: WindowHandle<Root>, cx: &mut App) {
    cx.on_action::<Quit>(|_, cx| cx.quit());
    cx.on_action::<ToggleTheme>(|_, cx| theme::toggle(cx));

    {
        let r = root.clone();
        cx.on_action::<Open>(move |_, cx| open_project(r.clone(), cx));
    }

    cx.on_action::<About>(|_, _| {
        let _ = notify::send(&notify::Notification::new(
            "Asylum",
            format!(
                "Agent Development Environment · v{}",
                env!("CARGO_PKG_VERSION")
            ),
        ));
    });

    cx.on_action::<Documentation>(|_, _| open_url("https://github.com/wess/asylum"));
    cx.on_action::<OpenLogFolder>(|_, cx| cx.reveal_path(&logs::default_dir()));

    // Settings remains available during onboarding so executable paths can be
    // corrected before the first repository is opened.
    cx.on_action::<OpenSettings>(move |_, cx| {
        window
            .update(cx, |root, w, cx| {
                if root.is_empty() {
                    root.onboarding_settings = true;
                    cx.notify();
                } else {
                    root.open_view(View::Settings, w, cx);
                    cx.notify();
                }
            })
            .ok();
    });

    {
        let root = root.clone();
        cx.on_action::<OpenSettingsFile>(move |_, cx| {
            if let Err(error) = open_settings_file(cx) {
                root.update(cx, |root, cx| {
                    root.push_error("Could not open settings", error);
                    cx.notify();
                });
            }
        });
    }

    cx.on_action::<RunFanout>(move |_, cx| {
        window
            .update(cx, |root, w, cx| {
                root.run_fanout(w, cx);
                cx.notify();
            })
            .ok();
    });

    on_view::<GoTasks>(window, View::Tasks, cx);
    on_view::<GoDiff>(window, View::Diff, cx);
    on_view::<GoSearch>(window, View::Search, cx);
    on_view::<FindInProject>(window, View::Search, cx);
    on_view::<GoIntegrations>(window, View::Integrations, cx);
    on_view::<GoTerminal>(window, View::Terminal, cx);
    on_view::<NewTerminal>(window, View::Terminal, cx);
    on_view::<GoEditor>(window, View::Editor, cx);
    on_view::<GoBrowser>(window, View::Browser, cx);
    on_view::<GoPreview>(window, View::Preview, cx);
    on_view::<GoPlugins>(window, View::Plugins, cx);
    on_view::<GoAccounts>(window, View::Accounts, cx);
    on_view::<GoInbox>(window, View::Notifications, cx);

    // New Task opens the Tasks tab and focuses its compose input directly,
    // rather than leaving the user to find and click into it.
    cx.on_action::<NewTask>(move |_, cx| {
        window
            .update(cx, |root, w, cx| {
                root.open_view(View::Tasks, w, cx);
                if let Some(compose) = root.compose.clone() {
                    w.focus(&compose.read(cx).focus_handle(), cx);
                }
                cx.notify();
            })
            .ok();
    });

    cx.on_action::<SplitRight>(move |_, cx| {
        window
            .update(cx, |root, w, cx| {
                root.split_terminal_pane(w, cx);
                cx.notify();
            })
            .ok();
    });

    cx.on_action::<CloseTab>(move |_, cx| {
        window
            .update(cx, |root, _w, cx| {
                let ap = root.workspace.active_pane;
                if let Some(pane) = root.workspace.panes.get(ap) {
                    let at = pane.active;
                    root.workspace.close(ap, at);
                }
                cx.notify();
            })
            .ok();
    });

    cx.on_action::<CommandPalette>(move |_, cx| {
        window
            .update(cx, |root, w, cx| {
                root.ensure_palettes(cx);
                if let Some(p) = root.palette.clone() {
                    p.update(cx, |sp, cx| sp.open(w, cx));
                }
            })
            .ok();
    });

    cx.on_action::<QuickOpen>(move |_, cx| {
        window
            .update(cx, |root, w, cx| {
                root.ensure_palettes(cx);
                if let Some(p) = root.quickopen.clone() {
                    p.update(cx, |sp, cx| sp.open(w, cx));
                }
            })
            .ok();
    });
}

/// Register an action that opens (or focuses) a [`View`]'s tab.
fn on_view<A: gpui::Action>(window: WindowHandle<Root>, view: View, cx: &mut App) {
    cx.on_action::<A>(move |_, cx| {
        window
            .update(cx, |root, w, cx| {
                root.open_view(view, w, cx);
                cx.notify();
            })
            .ok();
    });
}

/// Open settings.json in the system editor, seeding a starter file if needed.
pub fn open_settings_file(cx: &mut App) -> Result<(), String> {
    let path = config::default_path();
    config::edit::ensure_file(&path).map_err(|error| error.to_string())?;
    cx.open_with_system(&path);
    Ok(())
}

/// Open a URL in the system browser.
fn open_url(url: &str) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(url).spawn();
}
