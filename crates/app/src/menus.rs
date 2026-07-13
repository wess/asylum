//! The native macOS menu bar and its actions.
//!
//! Menu items dispatch gpui [`Action`]s; the same actions are bound to keyboard
//! shortcuts (so the menu shows the shortcut *and* the key works), and handled
//! globally against the root window. A standard macOS menu bar adapted to the
//! ADE's surfaces (File/View for tasks/terminals/tabs).

use gpui::{
    actions, App, Entity, KeyBinding, Menu, MenuItem, OsAction, WindowHandle,
};

use crate::root::open_project;
use crate::state::{Root, View};
use crate::theme;

actions!(
    asylum,
    [
        // App
        About,
        OpenSettings,
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
        // Help
        Documentation,
    ]
);

/// Default settings file contents, written when the user first opens Settings.
const DEFAULT_SETTINGS: &str = include_str!("../../../assets/settings.example.json");

/// Install the menu bar, keybindings, and action handlers.
pub fn install(root: Entity<Root>, window: WindowHandle<Root>, cx: &mut App) {
    cx.set_menus(menus());
    bind_keys(cx);
    register(root, window, cx);
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
                MenuItem::action("Diff Review", GoDiff),
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
            items: vec![MenuItem::action("Asylum Documentation", Documentation)],
        },
    ]
}

fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-n", NewTask, None),
        KeyBinding::new("cmd-o", Open, None),
        KeyBinding::new("cmd-t", NewTerminal, None),
        KeyBinding::new("cmd-d", SplitRight, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("cmd-k", CommandPalette, None),
        KeyBinding::new("cmd-p", QuickOpen, None),
        KeyBinding::new("cmd-f", FindInProject, None),
        KeyBinding::new("cmd-,", OpenSettings, None),
        KeyBinding::new("cmd-shift-t", ToggleTheme, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
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

    cx.on_action::<OpenSettings>(|_, cx| {
        let path = config::default_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if !path.exists() {
            let _ = std::fs::write(&path, DEFAULT_SETTINGS);
        }
        cx.open_with_system(&path);
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
    on_view::<NewTask>(window, View::Tasks, cx);

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

/// Open a URL in the system browser.
fn open_url(url: &str) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(url).spawn();
}
