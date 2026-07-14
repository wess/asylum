//! The root view: composes the ADE frame with guise's [`AppShell`] - a header,
//! an activity switcher + project/task navbar, the active surface in the main
//! area, and a status footer.

use gpui::prelude::*;
use gpui::{
    div, px, App, Entity, IntoElement, MouseButton, PathPromptOptions, SharedString, Window,
    WindowControlArea,
};

/// Left clearance in the titlebar for the macOS traffic lights.
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHT_INSET: f32 = 76.0;
#[cfg(not(target_os = "macos"))]
const TRAFFIC_LIGHT_INSET: f32 = 8.0;
use guise::prelude::*;
use libsinclair::terminal::SessionOptions;
use libsinclair::termview::{TermOptions, TermView};

use crate::icons::icon;
use crate::state::{Root, View};
use crate::workspace::TabKind;
use crate::{accounts, diff, fleet, integrations, notifications, search, sidebar, theme};

/// Open the native folder picker and add the chosen git repo as a project.
pub fn open_project(handle: Entity<Root>, cx: &mut App) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("Open".into()),
    });
    cx.spawn(async move |cx| {
        if let Ok(Ok(Some(paths))) = rx.await {
            if let Some(path) = paths.into_iter().next() {
                handle.update(cx, |root, cx| {
                    if let Err(e) = root.add_project_from_path(path) {
                        eprintln!("open project: {e}");
                    }
                    cx.notify();
                });
            }
        }
    })
    .detach();
}

/// The first-run / no-projects onboarding screen.
fn onboarding(handle: Entity<Root>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
        .gap_3()
        .child(icon("git-branch", 40.0).text_color(gpui::rgb(0x3b82f6)))
        .child(Title::new("Welcome to Asylum").order(1))
        .child(
            Text::new("Run a fleet of agents in isolated git worktrees.")
                .size(Size::Sm)
                .dimmed(),
        )
        .child(
            Text::new("Open a git repo, or any folder — we'll set up git for you.")
                .size(Size::Xs)
                .dimmed(),
        )
        .child(
            Button::new("open-project", "Open a folder…")
                .variant(Variant::Filled)
                .size(Size::Md)
                .on_click(move |_, _, cx| open_project(handle.clone(), cx)),
        )
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let project_id = self.project_id;
        let task_id = self.task_id;
        let handle = cx.entity();

        // No projects yet → onboarding.
        if self.is_empty() {
            return onboarding(handle).into_any_element();
        }

        let unread = self.unread();
        let tree = self.tree();
        let counts = (
            tree.len(),
            tree.iter().find(|p| Some(p.id) == project_id).map(|p| p.tasks.len()).unwrap_or(0),
            self.runs().len(),
        );
        let active_view = self.workspace.active_key().map(view_for_key);

        // Ensure the shared inputs exist.
        self.ensure_palettes(cx);
        crate::settings::ensure_inputs(self, cx);
        if self.compose.is_none() {
            self.compose = Some(cx.new(|cx| {
                guise::TextInput::new(cx).placeholder("Describe a task… e.g. Add a dark-mode toggle")
            }));
        }
        if self.review_note.is_none() {
            self.review_note =
                Some(cx.new(|cx| guise::TextInput::new(cx).placeholder("Add a review comment…")));
        }
        if self.design_note.is_none() {
            self.design_note =
                Some(cx.new(|cx| guise::TextInput::new(cx).placeholder("What should change here?")));
        }
        let palette = self.palette.clone().unwrap();
        let quickopen = self.quickopen.clone().unwrap();
        let compose = self.compose.clone().unwrap();
        let review_note = self.review_note.clone().unwrap();

        // Theme colors (owned, so the immutable theme borrow ends here).
        let colors = Colors::from(cx);

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

        let shell = AppShell::new()
            .header(40.0, {
                let palette = palette.clone();
                let quickopen = quickopen.clone();
                let h = handle.clone();
                move |_window, cx| {
                    header(palette.clone(), quickopen.clone(), h.clone(), Colors::from(cx))
                }
            })
            .navbar(280.0, {
                let handle = handle.clone();
                move |window, cx| {
                    sidebar::navbar(
                        active_view,
                        unread,
                        tree.clone(),
                        project_id,
                        task_id,
                        handle.clone(),
                        window,
                        cx,
                    )
                }
            })
            .footer(28.0, move |_window, _cx| footer(counts, unread))
            .child(main)
            .child(palette)
            .child(quickopen);

        // Wrap so the context menu can overlay the whole window.
        let mut root = div().size_full().child(shell);
        if let Some(menu) = self.context_menu.clone() {
            root = root.child(menu);
        }
        root.into_any_element()
    }
}

/// A render-time snapshot of a pane (owned; the workspace borrow is released).
struct PaneSnap {
    index: usize,
    active: bool,
    kind: Option<crate::workspace::TabKind>,
    tabs: Vec<TabSnap>,
}

struct TabSnap {
    index: usize,
    id: u64,
    title: String,
    icon: &'static str,
    active: bool,
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
        K::Integrations => View::Integrations,
        K::Accounts => View::Accounts,
        K::Inbox => View::Notifications,
        K::Plugins => View::Plugins,
        K::Settings => View::Settings,
        K::Terminal => View::Terminal,
        K::Editor => View::Editor,
        K::Browser => View::Browser,
        K::Preview => View::Preview,
    }
}

/// One pane: a tab bar over the active tab's content.
fn pane_view(
    snap: PaneSnap,
    content: gpui::AnyElement,
    handle: Entity<Root>,
    colors: &Colors,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(120.0))
        .h_full()
        .overflow_hidden()
        .when(!snap.active, |d| d.border_r_1().border_color(colors.border))
        .child(tab_bar(&snap, handle, colors))
        .child(div().flex_1().overflow_hidden().child(content))
}

/// A pane's tab strip: each tab (icon + title + close), plus a split button.
fn tab_bar(snap: &PaneSnap, handle: Entity<Root>, colors: &Colors) -> impl IntoElement {
    let pane = snap.index;
    let mut bar = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .w_full()
        .px(px(6.0))
        .py(px(4.0))
        .bg(colors.surface)
        .border_b_1()
        .border_color(colors.border);

    for tab in &snap.tabs {
        let ti = tab.index;
        let icon_color = if tab.active { colors.primary } else { colors.dimmed };
        let text_color = if tab.active { colors.text } else { colors.dimmed };
        let activate = handle.clone();
        let close = handle.clone();

        let mut chip = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .pl(px(8.0))
            .pr(px(4.0))
            .py(px(3.0))
            .rounded(px(6.0))
            .cursor_pointer();
        if tab.active {
            chip = chip.bg(colors.hover);
        }
        bar = bar.child(
            chip.child(
                div()
                    .id(SharedString::from(format!("tab-{}", tab.id)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(icon(tab.icon, 13.0).text_color(icon_color))
                    .child(
                        div()
                            .text_color(text_color)
                            .text_size(px(12.5))
                            .child(SharedString::from(tab.title.clone())),
                    )
                    .on_click(move |_, _, cx| {
                        activate.update(cx, |root, cx| {
                            root.workspace.activate(pane, ti);
                            cx.notify();
                        });
                    }),
            )
            .child(
                div()
                    .id(SharedString::from(format!("tabx-{}", tab.id)))
                    .px(px(3.0))
                    .rounded(px(4.0))
                    .text_color(colors.dimmed)
                    .text_size(px(12.0))
                    .cursor_pointer()
                    .child("×")
                    .on_click(move |_, _, cx| {
                        close.update(cx, |root, cx| {
                            root.workspace.close(pane, ti);
                            cx.notify();
                        });
                    }),
            ),
        );
    }

    // Split button: opens a terminal in a new pane to the right.
    let split = handle.clone();
    bar = bar.child(
        div()
            .id(SharedString::from(format!("split-{pane}")))
            .ml_auto()
            .px(px(5.0))
            .py(px(2.0))
            .rounded(px(5.0))
            .cursor_pointer()
            .child(icon("plus", 14.0).text_color(colors.dimmed))
            .on_click(move |_, window, cx| {
                split.update(cx, |root, cx| {
                    root.workspace.active_pane = pane;
                    root.split_terminal_pane(window, cx);
                    cx.notify();
                });
            }),
    );
    bar
}

impl Root {
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
                self.runs(),
                self.fanout.clone(),
                compose.clone(),
                handle,
                window,
                cx,
            )
            .into_any_element(),
            TabKind::Diff => diff::review(
                self.review_diff(),
                self.check_results.clone(),
                self.review_annotations(),
                self.review_target.clone(),
                self.branches(),
                review_note.clone(),
                handle,
                window,
                cx,
            )
            .into_any_element(),
            TabKind::Search => search::search_view(
                self.search_query.clone(),
                self.search_results.clone(),
                handle,
                window,
                cx,
            )
            .into_any_element(),
            TabKind::Integrations => integrations::integrations_view(
                self.prs.clone(),
                self.issues.clone(),
                self.integration_error.clone(),
                handle,
                window,
                cx,
            )
            .into_any_element(),
            TabKind::Accounts => {
                accounts::accounts_view(self.accounts(), handle, window, cx).into_any_element()
            }
            TabKind::Inbox => {
                notifications::inbox_view(self.notifications(), handle, window, cx).into_any_element()
            }
            TabKind::Plugins => {
                crate::plugins::plugins_view(self.plugins(), self.plugins_dir(), window, cx)
                    .into_any_element()
            }
            TabKind::Settings => match self.settings_inputs.clone() {
                Some(inputs) => crate::settings::settings_view(
                    self.settings.clone(),
                    self.settings_diagnostics.clone(),
                    inputs,
                    handle,
                    window,
                    cx,
                )
                .into_any_element(),
                None => Text::new("Settings loading…").dimmed().into_any_element(),
            },
            TabKind::Terminal(e) => e.clone().into_any_element(),
            TabKind::Editor(e, _) => e.clone().into_any_element(),
            TabKind::Browser(e) | TabKind::Preview(e) => match self.design_note.clone() {
                Some(note) => crate::browser::design_surface(
                    e.clone(),
                    self.design_enabled.contains(&e.entity_id()),
                    self.pending_capture.clone(),
                    self.design_annotations.clone(),
                    note,
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

    /// Build the command palette and quick-open overlays once. The palette lists
    /// view-switch and action commands; quick-open lists the project's files.
    pub fn ensure_palettes(&mut self, cx: &mut Context<Self>) {
        if self.palette.is_none() {
            let handle = cx.entity();
            let palette = cx.new(|cx| {
                let mut s = Spotlight::new(cx);
                for (view, _glyph, label) in View::BAR {
                    let view = *view;
                    let h = handle.clone();
                    s = s.item(format!("Go to {label}"), move |window, cx| {
                        h.update(cx, |root, cx| {
                            root.open_view(view, window, cx);
                            cx.notify();
                        });
                    });
                }
                let h = handle.clone();
                s = s.item("Run fan-out", move |_, cx| {
                    h.update(cx, |root, cx| {
                        root.run_fanout();
                        cx.notify();
                    });
                });
                let h = handle.clone();
                s = s.item("Run checks", move |_, cx| {
                    h.update(cx, |root, cx| {
                        root.run_checks();
                        cx.notify();
                    });
                });
                let h = handle.clone();
                s = s.item("Open Settings", move |window, cx| {
                    h.update(cx, |root, cx| {
                        root.open_view(View::Settings, window, cx);
                        cx.notify();
                    });
                });
                s = s.item("Open settings.json", move |_, cx| {
                    crate::menus::open_settings_file(cx);
                });
                s.item("Toggle theme", move |_, cx| crate::theme::toggle(cx))
            });
            self.palette = Some(palette);
        }

        if self.quickopen.is_none() {
            let handle = cx.entity();
            let files = self.project_files();
            let quickopen = cx.new(|cx| {
                let mut s = Spotlight::new(cx);
                for name in files {
                    let h = handle.clone();
                    let file = name.clone();
                    s = s.item(name, move |_, cx| {
                        let file = file.clone();
                        h.update(cx, |root, cx| {
                            root.open_file(&file, cx);
                            cx.notify();
                        });
                    });
                }
                s
            });
            self.quickopen = Some(quickopen);
        }
    }

}

/// The titlebar. With a transparent native titlebar, this *is* the window's top
/// chrome: the macOS traffic lights float into the left inset, the brand sits
/// beside them, a draggable filler moves the window (double-click zooms), and
/// the actions live on the right.
fn header(
    palette: Entity<guise::overlay::Spotlight>,
    quickopen: Entity<guise::overlay::Spotlight>,
    handle: Entity<Root>,
    colors: Colors,
) -> impl IntoElement {
    // A compact icon button for the titlebar's right cluster.
    let text = colors.text;
    let hover = colors.hover;
    let icon_btn = move |id: &'static str, name: &'static str| {
        div()
            .id(SharedString::from(id))
            .flex()
            .items_center()
            .justify_center()
            .w(px(28.0))
            .h(px(24.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .child(icon(name, 15.0).text_color(text))
    };
    let palette_btn = icon_btn("tb-palette", "command");
    let search_btn = icon_btn("tb-quickopen", "search");
    let open_btn = icon_btn("tb-open", "folder");
    let theme_btn = icon_btn("tb-theme", "sun");

    div()
        .id("titlebar")
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .w_full()
        .h_full()
        .pl(px(TRAFFIC_LIGHT_INSET))
        .pr(px(10.0))
        // The whole bar drags the window (double-click zooms); the buttons on
        // top handle their own clicks - a press with no movement isn't a drag.
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
        // Brand.
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(icon("git-branch", 15.0).text_color(colors.primary))
                .child(Title::new("Asylum").order(5)),
        )
        // Actions.
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .child(palette_btn.on_click(move |_, window, cx| {
                    palette.update(cx, |sp, cx| sp.open(window, cx));
                }))
                .child(search_btn.on_click(move |_, window, cx| {
                    quickopen.update(cx, |sp, cx| sp.open(window, cx));
                }))
                .child(open_btn.on_click(move |_, _, cx| open_project(handle.clone(), cx)))
                .child(theme_btn.on_click(|_, _, cx: &mut App| theme::toggle(cx))),
        )
}

/// The bottom status strip.
fn footer(counts: (usize, usize, usize), unread: usize) -> impl IntoElement {
    let (projects, tasks, runs) = counts;
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .w_full()
        .h_full()
        .px(px(16.0))
        .child(
            Text::new(SharedString::from(format!(
                "{projects} projects · {tasks} tasks · {runs} runs · {unread} unread"
            )))
            .size(Size::Xs)
            .dimmed(),
        )
}
