//! The primary surfaces ([`View`]) and how each opens into the workspace.

use gpui::prelude::*;
use gpui::{Context, Focusable, Window};
use libsinclair::termview::{TermOptions, TermView};

use crate::state::Root;
use crate::workspace::TabKind;

/// Which primary surface the main area shows. The activity bar switches these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// The fan-out board of per-agent run cards.
    Tasks,
    /// Annotatable diff review for the selected run.
    Diff,
    /// Cross-worktree content search.
    Search,
    /// Project Markdown knowledge, links, and task/run context.
    Notes,
    /// GitHub / Linear browsers.
    Integrations,
    /// Provider accounts + usage.
    Accounts,
    /// Notification inbox.
    Notifications,
    /// An embedded terminal in the selected project.
    Terminal,
    /// The built-in code editor with a file tree.
    Editor,
    /// Rich file preview (markdown rendered in a web view).
    Preview,
    /// Embedded browser (design-mode surface).
    Browser,
    /// Installed plugins.
    Plugins,
    /// The settings editor (writes back to settings.json).
    Settings,
}

impl View {
    /// Rail entries always shown: the core loop a new user needs first.
    pub const PRIMARY: &'static [View] = &[View::Tasks, View::Diff, View::Search];

    /// Rail entries behind the "More" reveal, in display order. Hidden by
    /// default; every one stays reachable from the command palette, menus, and
    /// keybindings, and an open hidden surface is surfaced by [`more_rail`].
    pub const MORE: &'static [View] = &[
        View::Notes,
        View::Integrations,
        View::Terminal,
        View::Editor,
        View::Preview,
        View::Browser,
        View::Plugins,
        View::Accounts,
        View::Notifications,
    ];

    /// The Lucide icon name for this view.
    pub fn icon(self) -> &'static str {
        match self {
            View::Tasks => "list-todo",
            View::Diff => "git-compare",
            View::Search => "search",
            View::Notes => "file-pen",
            View::Integrations => "github",
            View::Terminal => "terminal",
            View::Editor => "file-code",
            View::Preview => "eye",
            View::Browser => "globe",
            View::Plugins => "puzzle",
            View::Accounts => "circle-user",
            View::Notifications => "inbox",
            View::Settings => "settings",
        }
    }

    /// The settings.json keybinding action name bound to this view, if any
    /// (mirrors `menus::binding`'s vocabulary). `None` when the view has no
    /// bindable action yet, so the rail can only show a shortcut where one
    /// really exists.
    pub fn keymap_action(self) -> Option<&'static str> {
        match self {
            View::Tasks => Some("tasks"),
            View::Diff => Some("review_diff"),
            View::Search => Some("search"),
            View::Notes => None,
            View::Integrations => Some("integrations"),
            View::Terminal => Some("terminal"),
            View::Editor => Some("editor"),
            View::Preview => Some("preview"),
            View::Browser => Some("browser"),
            View::Plugins => Some("plugins"),
            View::Accounts => Some("switch_account"),
            View::Notifications => Some("notifications"),
            View::Settings => Some("settings"),
        }
    }

    /// The label for this view.
    pub fn label(self) -> &'static str {
        match self {
            View::Tasks => "Tasks",
            View::Diff => "Review",
            View::Search => "Search",
            View::Notes => "Notes",
            View::Integrations => "Integrations",
            View::Terminal => "Terminal",
            View::Editor => "Editor",
            View::Preview => "Preview",
            View::Browser => "Browser",
            View::Plugins => "Plugins",
            View::Accounts => "Accounts",
            View::Notifications => "Inbox",
            View::Settings => "Settings",
        }
    }
}

/// The "More" section as the rail should render it: the full set when
/// revealed; when hidden, just the active surface — opening a surface from the
/// palette, a menu, or a run card must never leave the current tab without a
/// rail entry.
pub fn more_rail(active: Option<View>, revealed: bool) -> Vec<View> {
    if revealed {
        View::MORE.to_vec()
    } else {
        active
            .filter(|view| View::MORE.contains(view))
            .into_iter()
            .collect()
    }
}

impl Root {
    /// Open the file `name` in a new editor tab.
    pub fn open_file(&mut self, name: &str, cx: &mut Context<Self>) {
        self.editor_file = Some(name.to_string());
        self.open_editor(name, cx);
    }

    // ── Tab opening ─────────────────────────────────────────────────────────

    /// Open (or focus) the tab for a nav-menu [`View`].
    pub fn open_view(&mut self, v: View, window: &mut Window, cx: &mut Context<Self>) {
        match v {
            View::Tasks => self.open_kind(TabKind::Tasks),
            View::Diff => self.open_kind(TabKind::Diff),
            View::Search => self.open_kind(TabKind::Search),
            View::Notes => self.open_kind(TabKind::Notes),
            View::Integrations => self.open_kind(TabKind::Integrations),
            View::Accounts => self.open_kind(TabKind::Accounts),
            View::Notifications => self.open_kind(TabKind::Inbox),
            View::Plugins => self.open_kind(TabKind::Plugins),
            View::Terminal => self.open_terminal(window, cx),
            View::Editor => {
                if let Some(f) = self.project_files().first().cloned() {
                    self.open_editor(&f, cx);
                } else {
                    self.push_notice(
                        crate::run::NoticeTone::Info,
                        "No files to open",
                        "This project has no top-level files for the editor to open (only its root directory is scanned).",
                    );
                }
            }
            View::Preview => self.open_preview(cx),
            View::Browser => self.open_browser(cx),
            View::Settings => self.open_kind(TabKind::Settings),
        }
    }

    pub(crate) fn open_kind(&mut self, kind: TabKind) {
        let id = self.next_tab_id();
        self.workspace.open(id, kind);
    }

    /// Reveal or hide the rail's "More" section, persisting the choice through
    /// settings.json (the file stays the source of truth; the reload applies
    /// it). Hiding removes the key, since hidden is the default. A failed write
    /// keeps the in-memory flip so the rail still responds, with an error notice.
    pub fn set_sidebar_more(&mut self, revealed: bool, cx: &mut Context<Self>) {
        self.settings.sidebar_more = revealed;
        let path = config::default_path();
        let result = if revealed {
            config::edit::set_key(&path, "sidebar_more", "true")
        } else {
            config::edit::remove_key(&path, "sidebar_more")
        };
        match result {
            Ok(()) => crate::reload::reload(self, cx),
            Err(error) => self.push_error("Could not save settings", error),
        }
        cx.notify();
    }

    /// Open a terminal tab running in the selected project.
    pub fn open_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self.project_path();
        let mut opts = libsinclair::terminal::SessionOptions::default();
        opts.spawn.cwd = Some(cwd.into());
        let term = cx.new(|cx| {
            TermView::spawn(opts, TermOptions::default(), window, cx).expect("spawn terminal")
        });
        let focus = term.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        let id = self.next_tab_id();
        self.workspace.open(id, TabKind::Terminal(term));
    }

    /// Open an editor tab for a project file.
    pub fn open_editor(&mut self, file: &str, cx: &mut Context<Self>) {
        let content = self.read_project_file(file);
        let is_rust = file.ends_with(".rs");
        let editor = cx.new(|cx| {
            let e = guise::Editor::new(cx).value(content.as_str());
            if is_rust {
                e.language(guise::Language::Rust)
            } else {
                e
            }
        });
        let id = self.next_tab_id();
        self.workspace
            .open(id, TabKind::Editor(editor, file.to_string()));
    }

    /// Open a browser tab with design mode on - click an element, attach a
    /// note, and collect numbered annotations for "send to agent".
    pub fn open_browser(&mut self, cx: &mut Context<Self>) {
        let wv = cx.new(|cx| {
            guise::WebView::new(cx)
                .init_script(designmode::INJECT_JS)
                .url("https://example.com")
        });
        self.design_enabled.insert(wv.entity_id());
        self.watch_design_messages(&wv, cx);
        let id = self.next_tab_id();
        self.workspace.open(id, TabKind::Browser(wv));
    }

    /// Open a preview tab (the open editor file, or the project README).
    /// Design mode is available from the toolbar but starts off.
    pub fn open_preview(&mut self, cx: &mut Context<Self>) {
        let html = self.preview_html();
        let wv = cx.new(|cx| {
            guise::WebView::new(cx)
                .init_script(designmode::INJECT_JS)
                .html(html)
        });
        self.watch_design_messages(&wv, cx);
        let id = self.next_tab_id();
        self.workspace.open(id, TabKind::Preview(wv));
    }

    /// Top-level files of the selected project worth opening in the editor
    /// (texty files and small config), sorted, capped.
    pub fn project_files(&self) -> Vec<String> {
        let dir = std::path::PathBuf::from(self.project_path());
        let mut files: Vec<String> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_file())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| !name.starts_with('.'))
            .collect();
        files.sort();
        files.truncate(200);
        files
    }

    /// Read a project file's contents for the editor (empty on error).
    pub fn read_project_file(&self, name: &str) -> String {
        let path = std::path::PathBuf::from(self.project_path()).join(name);
        std::fs::read_to_string(path).unwrap_or_default()
    }

    /// A full HTML preview document for the Preview surface: the file open in
    /// the editor (markdown / image / PDF / text), else the project README.
    pub fn preview_html(&self) -> String {
        let dir = std::path::PathBuf::from(self.project_path());
        if let Some(name) = &self.editor_file {
            if let Ok(html) = preview::html_document(&dir.join(name)) {
                return html;
            }
        }
        for candidate in ["README.md", "readme.md", "Readme.md"] {
            let path = dir.join(candidate);
            if path.exists() {
                if let Ok(html) = preview::html_document(&path) {
                    return html;
                }
            }
        }
        preview::html_document(std::path::Path::new("/nonexistent")).unwrap_or_else(|_| {
            "<!doctype html><p>Nothing to preview. Open a file in the editor.</p>".to_string()
        })
    }
}
