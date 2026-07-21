//! The root view: composes the ADE frame with guise's [`AppShell`] - a header,
//! an activity switcher + project/task navbar, the active surface in the main
//! area, and a status footer.

mod chrome;
mod palette;
mod panes;

pub use chrome::open_project;

use gpui::prelude::*;
use gpui::{div, px, App, DragMoveEvent, Entity, IntoElement, Window};
use guise::prelude::*;
use libsinclair::terminal::SessionOptions;
use libsinclair::termview::{TermOptions, TermView};

use chrome::{
    confirm_bar, footer, header, notice_stack, onboarding, onboarding_settings, sidebar_resizer,
};
use panes::{pane_view, PaneSnap, TabSnap};

use crate::state::{Root, View};
use crate::workspace::TabKind;
use crate::{accounts, diff, fleet, integrations, note, notifications, search, sidebar};

/// Min/max width (px) the left navigation can be dragged to.
const SIDEBAR_MIN: f32 = 180.0;
const SIDEBAR_MAX: f32 = 560.0;
/// Width (px) of the left navigation while collapsed to its icon rail.
const SIDEBAR_COLLAPSED: f32 = 52.0;

/// Header/footer chrome heights (px). The overlay layers below (the notice
/// stack, the confirm bar, the sidebar resize divider) anchor to these
/// instead of duplicating the numbers, so a chrome resize can't silently
/// reopen an overlap.
const HEADER_HEIGHT: f32 = 40.0;
const FOOTER_HEIGHT: f32 = 28.0;

/// Drag payload for the sidebar resize divider. A distinct type so `on_drag_move`
/// only reacts to this drag.
struct SidebarDrag;

/// Left clearance in the titlebar for the macOS traffic lights.
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHT_INSET: f32 = 76.0;
#[cfg(not(target_os = "macos"))]
const TRAFFIC_LIGHT_INSET: f32 = 8.0;

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.launch_needed {
            cx.defer_in(window, |root, window, cx| root.launch_queued(window, cx));
        }
        let project_id = self.project_id;
        let task_id = self.task_id;
        let handle = cx.entity();
        // Theme colors (owned, so the immutable theme borrow ends here).
        // Read early so both the onboarding screen and the main shell can
        // use the same snapshot.
        let colors = Colors::from(cx);

        // No projects yet → onboarding.
        if self.is_empty() {
            crate::settings::ensure_inputs(self, cx);
            if self.onboarding_settings {
                return onboarding_settings(
                    self.settings.clone(),
                    self.settings_diagnostics.clone(),
                    self.agent_rows(),
                    self.settings_inputs.clone().expect("settings inputs"),
                    self.settings_collapsed.clone(),
                    handle,
                    window,
                    cx,
                );
            }
            return onboarding(
                handle,
                self.pending_project.clone(),
                self.agent_reports(),
                self.notices.clone(),
                colors.primary,
            )
            .into_any_element();
        }

        let unread = self.unread();
        let tree = self.tree();
        let counts = (
            tree.len(),
            tree.iter()
                .find(|p| Some(p.id) == project_id)
                .map(|p| p.tasks.len())
                .unwrap_or(0),
            self.runs().len(),
        );
        let active_view = self.workspace.active_key().map(view_for_key);
        self.sync_webview_visibility(window, cx);

        // Ensure the shared inputs exist.
        self.ensure_palettes(cx);
        crate::settings::ensure_inputs(self, cx);
        if self.compose.is_none() {
            self.compose = Some(cx.new(|cx| {
                guise::TextInput::new(cx)
                    .placeholder("Describe a task… e.g. Add a dark-mode toggle")
            }));
        }
        if self.start_ref_input.is_none() {
            let input = cx.new(|cx| {
                guise::TextInput::new(cx).placeholder("Start from branch/commit (default: base)")
            });
            cx.subscribe(&input, |root, _input, event: &guise::TextInputEvent, cx| {
                let (guise::TextInputEvent::Change(value) | guise::TextInputEvent::Submit(value)) =
                    event;
                root.start_ref = value.clone();
                cx.notify();
            })
            .detach();
            self.start_ref_input = Some(input);
        }
        if self.review_note.is_none() {
            self.review_note =
                Some(cx.new(|cx| guise::TextInput::new(cx).placeholder("Add a review comment…")));
        }
        if self.account_input.is_none() {
            let input = cx.new(|cx| {
                guise::TextInput::new(cx).placeholder("Provider: label, e.g. claude: work")
            });
            cx.subscribe(&input, |root, _input, event: &guise::TextInputEvent, cx| {
                if matches!(event, guise::TextInputEvent::Submit(_)) {
                    root.add_account_from_input(cx);
                }
            })
            .detach();
            self.account_input = Some(input);
        }
        if self.design_note.is_none() {
            self.design_note = Some(
                cx.new(|cx| guise::TextInput::new(cx).placeholder("What should change here?")),
            );
        }
        self.ensure_notes(cx);
        crate::search::ensure_input(self, cx);
        let palette = self.palette.clone().unwrap();
        let quickopen = self.quickopen.clone().unwrap();
        let compose = self.compose.clone().unwrap();
        let review_note = self.review_note.clone().unwrap();

        // Snapshot the pane/tab layout, cloning each active tab's kind so the
        // workspace borrow is released before we build content.
        let active_pane = self.workspace.active_pane;
        let pane_snaps: Vec<PaneSnap> = self
            .workspace
            .panes
            .iter()
            .enumerate()
            .map(|(pi, pane)| PaneSnap {
                index: pi,
                active: pi == active_pane,
                kind: pane.active_tab().map(|t| t.kind.clone()),
                tabs: pane
                    .tabs
                    .iter()
                    .enumerate()
                    .map(|(ti, t)| TabSnap {
                        index: ti,
                        id: t.id,
                        title: t.kind.title(),
                        icon: t.kind.icon(),
                        active: ti == pane.active,
                    })
                    .collect(),
            })
            .collect();

        // Build the pane row.
        let mut area = div().flex().flex_row().size_full().overflow_hidden();
        for snap in pane_snaps {
            let content = match &snap.kind {
                Some(kind) => {
                    self.tab_content(kind, &compose, &review_note, handle.clone(), window, cx)
                }
                None => Text::new("Empty pane").dimmed().into_any_element(),
            };
            area = area.child(pane_view(snap, content, handle.clone(), &colors));
        }
        let main = area.into_any_element();

        let sidebar_extent = if self.sidebar_collapsed {
            SIDEBAR_COLLAPSED
        } else {
            self.sidebar_width
        };
        let shell = AppShell::new()
            .header(HEADER_HEIGHT, {
                let palette = palette.clone();
                let quickopen = quickopen.clone();
                let h = handle.clone();
                move |_window, cx| {
                    header(
                        palette.clone(),
                        quickopen.clone(),
                        h.clone(),
                        Colors::from(cx),
                    )
                }
            })
            .navbar(sidebar_extent, {
                let handle = handle.clone();
                let collapsed = self.sidebar_collapsed;
                let more_shown = self.settings.sidebar_more;
                let keymap = config::Keymap::from_settings(&self.settings.keybindings);
                move |window, cx| {
                    sidebar::navbar(
                        active_view,
                        unread,
                        tree.clone(),
                        project_id,
                        task_id,
                        collapsed,
                        more_shown,
                        &keymap,
                        handle.clone(),
                        window,
                        cx,
                    )
                }
            })
            .footer(FOOTER_HEIGHT, move |_window, _cx| footer(counts, unread))
            .child(main)
            .child(palette)
            .child(quickopen);

        // Wrap so the context menu can overlay the whole window. The wrapper also
        // hosts the sidebar resize divider and follows its drag.
        let mut root = div()
            .relative()
            .size_full()
            .on_drag_move(
                cx.listener(|this, ev: &DragMoveEvent<SidebarDrag>, _window, cx| {
                    let x = f32::from(ev.event.position.x - ev.bounds.left());
                    let next = x.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
                    if (next - this.sidebar_width).abs() > f32::EPSILON {
                        this.sidebar_width = next;
                        cx.notify();
                    }
                }),
            )
            .child(shell);
        // A thin draggable divider on the navbar's right edge (expanded only).
        if !self.sidebar_collapsed {
            root = root.child(sidebar_resizer(sidebar_extent));
        }
        if !self.notices.is_empty() {
            root = root.child(notice_stack(self.notices.clone(), handle.clone()));
        }
        if let Some(action) = self.confirm.clone() {
            root = root.child(confirm_bar(action, sidebar_extent, &colors, handle.clone()));
        }
        if let Some(menu) = self.context_menu.clone() {
            root = root.child(menu);
        }
        root.into_any_element()
    }
}

/// Theme colors captured once per render.
struct Colors {
    text: gpui::Hsla,
    dimmed: gpui::Hsla,
    primary: gpui::Hsla,
    hover: gpui::Hsla,
    border: gpui::Hsla,
    surface: gpui::Hsla,
}

impl Colors {
    fn from(cx: &App) -> Self {
        let t = guise::theme::theme(cx);
        Colors {
            text: t.text().hsla(),
            dimmed: t.dimmed().hsla(),
            primary: t.primary().hsla(),
            hover: t.surface_hover().hsla(),
            border: t.border().hsla(),
            surface: t.surface().hsla(),
        }
    }
}

/// Map an active tab key back to the nav-menu [`View`] for highlighting.
fn view_for_key(key: crate::workspace::TabKey) -> View {
    use crate::workspace::TabKey as K;
    match key {
        K::Tasks => View::Tasks,
        K::Diff => View::Diff,
        K::Search => View::Search,
        K::Notes => View::Notes,
        K::Integrations => View::Integrations,
        K::Accounts => View::Accounts,
        K::Inbox => View::Notifications,
        K::Plugins => View::Plugins,
        K::Settings => View::Settings,
        K::Terminal => View::Terminal,
        K::Run => View::Tasks,
        K::Editor => View::Editor,
        K::Browser => View::Browser,
        K::Preview => View::Preview,
    }
}

impl Root {
    fn sync_webview_visibility(&self, window: &Window, cx: &mut App) {
        let mut notes_visible = false;
        for pane in &self.workspace.panes {
            for (index, tab) in pane.tabs.iter().enumerate() {
                let visible = index == pane.active;
                match &tab.kind {
                    TabKind::Browser(webview) | TabKind::Preview(webview) => {
                        webview.update(cx, |webview, _cx| webview.set_visible(visible));
                    }
                    TabKind::Notes if visible => notes_visible = true,
                    _ => {}
                }
            }
        }

        let compact = window.viewport_size().width < px(1050.0);
        let note_preview_visible = notes_visible
            && self.note.view != note::Mode::Edit
            && (!compact || self.note.panel == note::Panel::Write);
        if let Some(preview) = self.note.preview.clone() {
            preview.update(cx, |preview, _cx| preview.set_visible(note_preview_visible));
        }
    }

    /// The selected project's name, for the compose box header.
    fn project_name(&self) -> String {
        self.project_id
            .and_then(|id| self.db.project(id).ok())
            .map(|p| p.name)
            .unwrap_or_default()
    }

    /// Render the content for a tab's kind.
    fn tab_content(
        &self,
        kind: &TabKind,
        compose: &Entity<guise::TextInput>,
        review_note: &Entity<guise::TextInput>,
        handle: Entity<Root>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        match kind {
            TabKind::Tasks => fleet::main_content(
                self.project_name(),
                self.task_title(),
                self.task_status(),
                self.task_id,
                self.runs(),
                self.fanout.clone(),
                self.agent_reports(),
                self.composer_advanced,
                self.show_all_agents,
                self.fanout_in_progress,
                self.setup_checks.clone(),
                self.setup_open,
                // Read off `self`, not through the handle: this runs inside
                // Root's own render, so `handle.read(cx)` double-leases it.
                self.layout_names(),
                compose.clone(),
                self.start_ref_input.clone().unwrap(),
                handle,
                window,
                cx,
            )
            .into_any_element(),
            TabKind::Diff => diff::review(
                self.review_diff(),
                self.current_run_id()
                    .map(|id| self.run_check_results(id))
                    .unwrap_or_default(),
                self.current_run_id()
                    .is_some_and(|id| self.checking_runs.contains(&id)),
                self.review_annotations(),
                self.review_target.clone(),
                self.branches(),
                self.runs(),
                self.diff_split,
                self.review_staging(),
                review_note.clone(),
                handle,
                window,
                cx,
            )
            .into_any_element(),
            TabKind::Search => search::search_view(
                self.search_query.clone(),
                self.search_results.clone(),
                self.search_input.clone().expect("search input"),
                handle,
                window,
                cx,
            )
            .into_any_element(),
            TabKind::Notes => note::surface(self, handle, window, cx).into_any_element(),
            TabKind::Integrations => integrations::integrations_view(
                self.prs.clone(),
                self.issues.clone(),
                self.linear_issues.clone(),
                !self.settings.linear_token.trim().is_empty(),
                self.integration_error.clone(),
                handle,
                window,
                cx,
            )
            .into_any_element(),
            TabKind::Accounts => accounts::accounts_view(
                self.accounts(),
                self.account_input.clone().unwrap(),
                handle,
                window,
                cx,
            )
            .into_any_element(),
            TabKind::Inbox => notifications::inbox_view(self.notifications(), handle, window, cx)
                .into_any_element(),
            TabKind::Plugins => crate::plugins::plugins_view(
                self.plugins(),
                self.plugins_dir(),
                self.settings.enabled_plugins.clone(),
                handle,
                window,
                cx,
            )
            .into_any_element(),
            TabKind::Settings => match self.settings_inputs.clone() {
                Some(inputs) => crate::settings::settings_view(
                    self.settings.clone(),
                    self.settings_diagnostics.clone(),
                    self.agent_rows(),
                    inputs,
                    self.settings_collapsed.clone(),
                    handle,
                    window,
                    cx,
                )
                .into_any_element(),
                None => Text::new("Settings loading…").dimmed().into_any_element(),
            },
            TabKind::Terminal(e) => e.clone().into_any_element(),
            TabKind::Run(id) => {
                crate::fleet::run_terminal(*id, self, handle, window, cx).into_any_element()
            }
            TabKind::Editor(e, _) => e.clone().into_any_element(),
            TabKind::Browser(e) | TabKind::Preview(e) => match self.design_note.clone() {
                Some(note) => crate::browser::design_surface(
                    e.clone(),
                    self.design_enabled.contains(&e.entity_id()),
                    self.pending_capture.clone(),
                    self.design_annotations.clone(),
                    note,
                    guise::theme::theme(cx).primary().hsla(),
                    handle,
                )
                .into_any_element(),
                None => e.clone().into_any_element(),
            },
        }
    }

    /// Open a terminal in a new pane split to the right.
    pub fn split_terminal_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self.project_path();
        let mut opts = SessionOptions::default();
        opts.spawn.cwd = Some(cwd.into());
        let term = cx.new(|cx| {
            TermView::spawn(opts, TermOptions::default(), window, cx).expect("spawn terminal")
        });
        let id = self.next_tab_id();
        self.workspace.split(id, TabKind::Terminal(term));
    }
}
