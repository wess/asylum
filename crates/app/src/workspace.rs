//! The tabbed, splittable main-area workspace.
//!
//! The main area isn't a single screen — it's a set of **panes** laid out
//! side by side, each holding **tabs** (an agent terminal, a plain terminal, an
//! editor, a browser, a diff, …). This module is the pure model of that: a
//! [`Workspace`] owns a row of [`Pane`]s, each pane owns [`Tab`]s. Opening a
//! surface adds a tab to the active pane; splitting adds a pane. The gpui view
//! (`root.rs`) renders it and the surface builders fill each tab.

use gpui::Entity;
use guise::{Editor, WebView};
use libsinclair::termview::TermView;

/// What a tab shows. Data surfaces (Tasks, Diff, …) render from live [`Root`]
/// state; entity surfaces carry their gpui entity so it persists across frames.
#[derive(Clone)]
pub enum TabKind {
    Tasks,
    Diff,
    Search,
    Integrations,
    Accounts,
    Inbox,
    Plugins,
    Terminal(Entity<TermView>),
    Editor(Entity<Editor>, String),
    Browser(Entity<WebView>),
    Preview(Entity<WebView>),
}

/// A stable discriminant for a [`TabKind`], for "focus existing or open new".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabKey {
    Tasks,
    Diff,
    Search,
    Integrations,
    Accounts,
    Inbox,
    Plugins,
    Terminal,
    Editor,
    Browser,
    Preview,
}

impl TabKind {
    pub fn key(&self) -> TabKey {
        match self {
            TabKind::Tasks => TabKey::Tasks,
            TabKind::Diff => TabKey::Diff,
            TabKind::Search => TabKey::Search,
            TabKind::Integrations => TabKey::Integrations,
            TabKind::Accounts => TabKey::Accounts,
            TabKind::Inbox => TabKey::Inbox,
            TabKind::Plugins => TabKey::Plugins,
            TabKind::Terminal(_) => TabKey::Terminal,
            TabKind::Editor(..) => TabKey::Editor,
            TabKind::Browser(_) => TabKey::Browser,
            TabKind::Preview(_) => TabKey::Preview,
        }
    }

    /// The Lucide icon name for the tab.
    pub fn icon(&self) -> &'static str {
        match self.key() {
            TabKey::Tasks => "list-todo",
            TabKey::Diff => "git-compare",
            TabKey::Search => "search",
            TabKey::Integrations => "github",
            TabKey::Accounts => "circle-user",
            TabKey::Inbox => "inbox",
            TabKey::Plugins => "puzzle",
            TabKey::Terminal => "terminal",
            TabKey::Editor => "file-pen",
            TabKey::Browser => "globe",
            TabKey::Preview => "eye",
        }
    }

    /// The tab's title.
    pub fn title(&self) -> String {
        match self {
            TabKind::Tasks => "Tasks".into(),
            TabKind::Diff => "Diff".into(),
            TabKind::Search => "Search".into(),
            TabKind::Integrations => "Integrations".into(),
            TabKind::Accounts => "Accounts".into(),
            TabKind::Inbox => "Inbox".into(),
            TabKind::Plugins => "Plugins".into(),
            TabKind::Terminal(_) => "Terminal".into(),
            TabKind::Editor(_, file) => file
                .rsplit('/')
                .next()
                .unwrap_or(file)
                .to_string(),
            TabKind::Browser(_) => "Browser".into(),
            TabKind::Preview(_) => "Preview".into(),
        }
    }

    /// Whether only one tab of this kind should exist (data surfaces).
    fn singleton(&self) -> bool {
        !matches!(
            self.key(),
            TabKey::Terminal | TabKey::Editor | TabKey::Browser | TabKey::Preview
        )
    }
}

/// One tab in a pane.
#[derive(Clone)]
pub struct Tab {
    pub id: u64,
    pub kind: TabKind,
}

/// A pane: an ordered set of tabs with one active.
pub struct Pane {
    pub tabs: Vec<Tab>,
    pub active: usize,
}

impl Pane {
    fn new(tab: Tab) -> Self {
        Pane {
            tabs: vec![tab],
            active: 0,
        }
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }
}

/// The whole main-area layout: a row of panes.
pub struct Workspace {
    pub panes: Vec<Pane>,
    pub active_pane: usize,
}

impl Workspace {
    /// A fresh workspace with a single Tasks tab.
    pub fn new(first_id: u64) -> Self {
        Workspace {
            panes: vec![Pane::new(Tab {
                id: first_id,
                kind: TabKind::Tasks,
            })],
            active_pane: 0,
        }
    }

    /// The active pane's active tab kind key, for nav highlighting.
    pub fn active_key(&self) -> Option<TabKey> {
        self.panes
            .get(self.active_pane)
            .and_then(|p| p.active_tab())
            .map(|t| t.kind.key())
    }

    /// Open a tab. Singleton kinds focus an existing tab of that kind anywhere;
    /// otherwise a new tab is added to the active pane and focused.
    pub fn open(&mut self, id: u64, kind: TabKind) {
        if kind.singleton() {
            let key = kind.key();
            for (pi, pane) in self.panes.iter_mut().enumerate() {
                if let Some(ti) = pane.tabs.iter().position(|t| t.kind.key() == key) {
                    pane.active = ti;
                    self.active_pane = pi;
                    return;
                }
            }
        }
        let pane = &mut self.panes[self.active_pane];
        pane.tabs.push(Tab { id, kind });
        pane.active = pane.tabs.len() - 1;
    }

    /// Split: add a new pane (to the right of the active one) holding `kind`.
    pub fn split(&mut self, id: u64, kind: TabKind) {
        let idx = (self.active_pane + 1).min(self.panes.len());
        self.panes.insert(idx, Pane::new(Tab { id, kind }));
        self.active_pane = idx;
    }

    /// Activate a tab in a pane.
    pub fn activate(&mut self, pane_idx: usize, tab_idx: usize) {
        if let Some(pane) = self.panes.get_mut(pane_idx) {
            if tab_idx < pane.tabs.len() {
                pane.active = tab_idx;
                self.active_pane = pane_idx;
            }
        }
    }

    /// Close a tab. Empty panes are removed (but at least one pane remains).
    pub fn close(&mut self, pane_idx: usize, tab_idx: usize) {
        let Some(pane) = self.panes.get_mut(pane_idx) else {
            return;
        };
        if tab_idx >= pane.tabs.len() {
            return;
        }
        pane.tabs.remove(tab_idx);
        if pane.active >= pane.tabs.len() && !pane.tabs.is_empty() {
            pane.active = pane.tabs.len() - 1;
        }
        if pane.tabs.is_empty() && self.panes.len() > 1 {
            self.panes.remove(pane_idx);
        }
        self.active_pane = self.active_pane.min(self.panes.len().saturating_sub(1));
    }
}

#[cfg(test)]
#[path = "../tests/workspace.rs"]
mod tests;
