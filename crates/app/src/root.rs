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

use crate::control::Button;
use crate::icons::{icon, icon_button};
use crate::state::{Root, View};
use crate::workspace::TabKind;
use crate::{accounts, diff, fleet, integrations, note, notifications, search, sidebar, theme};

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
                    if let Err(error) = root.consider_project_path(path) {
                        root.push_error("Could not open folder", error);
                    }
                    cx.notify();
                });
            }
        }
    })
    .detach();
}

/// The first-run / no-projects onboarding screen.
fn onboarding(
    handle: Entity<Root>,
    pending: Option<std::path::PathBuf>,
    reports: Vec<(agent::registry::Agent, agent::doctor::Report)>,
    notices: Vec<crate::run::Notice>,
) -> impl IntoElement {
    let installed = reports.iter().filter(|(_, report)| report.ready()).count();
    let verified = reports
        .iter()
        .filter(|(_, report)| report.verified())
        .count();
    let mut body = div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
        .gap_3();
    for notice in notices {
        let color = match notice.tone {
            crate::run::NoticeTone::Error => ColorName::Red,
            crate::run::NoticeTone::Warning => ColorName::Orange,
            crate::run::NoticeTone::Success => ColorName::Green,
            crate::run::NoticeTone::Info => ColorName::Blue,
        };
        body = body.child(
            div().w(px(560.0)).child(
                Alert::new(SharedString::from(notice.message))
                    .title(SharedString::from(notice.title))
                    .color(color),
            ),
        );
    }
    if let Some(path) = pending {
        let initialize = handle.clone();
        let cancel = handle;
        return body
            .child(icon("git-branch", 40.0).text_color(gpui::rgb(0x3b82f6)))
            .child(Title::new("Initialize git?").order(1))
            .child(Text::new(SharedString::from(path.display().to_string())).size(Size::Sm))
            .child(
                Text::new("Asylum needs git worktrees. It will create a repository and an empty initial commit; existing files are not committed.")
                    .size(Size::Xs)
                    .dimmed(),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        Button::new("cancel-project", "Choose another folder")
                            .variant(Variant::Subtle)
                            .size(Size::Sm)
                            .on_click(move |_, _, cx| {
                                cancel.update(cx, |root, cx| {
                                    root.pending_project = None;
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        Button::new("initialize-project", "Initialize and open")
                            .variant(Variant::Filled)
                            .size(Size::Sm)
                            .on_click(move |_, _, cx| {
                                initialize.update(cx, |root, cx| {
                                    if let Err(error) = root.initialize_pending_project() {
                                        root.push_error("Could not initialize folder", error);
                                    }
                                    cx.notify();
                                });
                            }),
                    ),
            );
    }
    let configure = handle.clone();
    body.child(icon("git-branch", 40.0).text_color(gpui::rgb(0x3b82f6)))
        .child(Title::new("Welcome to Asylum").order(1))
        .child(
            Text::new("Open a repository to create your first isolated agent run.")
                .size(Size::Sm)
                .dimmed(),
        )
        .child(
            Badge::new(SharedString::from(format!(
                "{verified} verified, {installed} installed"
            )))
            .color(if verified > 0 {
                ColorName::Green
            } else {
                ColorName::Orange
            })
            .variant(Variant::Light),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(
                    Button::new("open-project", "Open a folder…")
                        .variant(Variant::Filled)
                        .size(Size::Md)
                        .on_click(move |_, _, cx| open_project(handle.clone(), cx)),
                )
                .child(
                    Button::new("configure-agents", "Configure agents")
                        .variant(Variant::Subtle)
                        .size(Size::Md)
                        .on_click(move |_, _, cx| {
                            configure.update(cx, |root, cx| {
                                root.onboarding_settings = true;
                                cx.notify();
                            });
                        }),
                ),
        )
}

fn onboarding_settings(
    settings: config::Settings,
    diagnostics: Vec<config::Diagnostic>,
    reports: Vec<(agent::registry::Agent, agent::doctor::Report)>,
    inputs: crate::settings::Inputs,
    handle: Entity<Root>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let back = handle.clone();
    div()
        .id("onboarding-settings")
        .flex()
        .flex_col()
        .size_full()
        .overflow_y_scroll()
        .child(
            div().p_3().child(
                Button::new("settings-back", "Back to setup")
                    .size(Size::Sm)
                    .variant(Variant::Subtle)
                    .on_click(move |_, _, cx| {
                        back.update(cx, |root, cx| {
                            root.onboarding_settings = false;
                            cx.notify();
                        });
                    }),
            ),
        )
        .child(crate::settings::settings_view(
            settings,
            diagnostics,
            reports,
            inputs,
            handle,
            window,
            cx,
        ))
        .into_any_element()
}

fn notice_stack(notices: Vec<crate::run::Notice>, handle: Entity<Root>) -> impl IntoElement {
    let mut stack = div()
        .absolute()
        .top(px(48.0))
        .right(px(12.0))
        .w(px(420.0))
        .flex()
        .flex_col()
        .gap_2();
    for notice in notices {
        let close = handle.clone();
        let id = notice.id;
        let color = match notice.tone {
            crate::run::NoticeTone::Info => ColorName::Blue,
            crate::run::NoticeTone::Success => ColorName::Green,
            crate::run::NoticeTone::Warning => ColorName::Orange,
            crate::run::NoticeTone::Error => ColorName::Red,
        };
        stack = stack.child(
            Alert::new(SharedString::from(notice.message))
                .title(SharedString::from(notice.title))
                .color(color)
                .on_close(move |_, _, cx| {
                    close.update(cx, |root, cx| {
                        root.dismiss_notice(id);
                        cx.notify();
                    });
                }),
        );
    }
    stack
}

fn confirm_bar(action: crate::run::ConfirmAction, handle: Entity<Root>) -> impl IntoElement {
    let confirm = handle.clone();
    let cancel = handle;
    div()
        .absolute()
        .bottom(px(38.0))
        .left(px(300.0))
        .right(px(12.0))
        .p_3()
        .border_1()
        .rounded(px(6.0))
        .bg(gpui::rgb(0x1f2937))
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(Text::new(action.title()).bold())
                .child(Text::new(action.message()).size(Size::Xs).dimmed()),
        )
        .child(
            Button::new("confirm-cancel", "Cancel")
                .size(Size::Sm)
                .variant(Variant::Subtle)
                .on_click(move |_, _, cx| {
                    cancel.update(cx, |root, cx| {
                        root.confirm = None;
                        cx.notify();
                    });
                }),
        )
        .child(
            Button::new("confirm-action", "Confirm")
                .size(Size::Sm)
                .variant(Variant::Filled)
                .on_click(move |_, _, cx| {
                    confirm.update(cx, |root, cx| {
                        root.confirm_action(cx);
                        cx.notify();
                    });
                }),
        )
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.launch_needed {
            cx.defer_in(window, |root, window, cx| root.launch_queued(window, cx));
        }
        let project_id = self.project_id;
        let task_id = self.task_id;
        let handle = cx.entity();

        // No projects yet → onboarding.
        if self.is_empty() {
            crate::settings::ensure_inputs(self, cx);
            if self.onboarding_settings {
                return onboarding_settings(
                    self.settings.clone(),
                    self.settings_diagnostics.clone(),
                    self.agent_reports(),
                    self.settings_inputs.clone().expect("settings inputs"),
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
        if self.review_note.is_none() {
            self.review_note =
                Some(cx.new(|cx| guise::TextInput::new(cx).placeholder("Add a review comment…")));
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
                    header(
                        palette.clone(),
                        quickopen.clone(),
                        h.clone(),
                        Colors::from(cx),
                    )
                }
            })
            .navbar(if self.sidebar_collapsed { 52.0 } else { 280.0 }, {
                let handle = handle.clone();
                let collapsed = self.sidebar_collapsed;
                move |window, cx| {
                    sidebar::navbar(
                        active_view,
                        unread,
                        tree.clone(),
                        project_id,
                        task_id,
                        collapsed,
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
        let mut root = div().relative().size_full().child(shell);
        if !self.notices.is_empty() {
            root = root.child(notice_stack(self.notices.clone(), handle.clone()));
        }
        if let Some(action) = self.confirm.clone() {
            root = root.child(confirm_bar(action, handle.clone()));
        }
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
        let icon_color = if tab.active {
            colors.primary
        } else {
            colors.dimmed
        };
        let text_color = if tab.active {
            colors.text
        } else {
            colors.dimmed
        };
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
                    .tab_index(0)
                    .role(gpui::accesskit::Role::Tab)
                    .aria_label(SharedString::from(tab.title.clone()))
                    .aria_selected(tab.active)
                    .focus_visible(move |style| style.border_1().border_color(colors.primary))
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
            .child(icon_button(
                SharedString::from(format!("tabx-{}", tab.id)),
                "circle-x",
                SharedString::from(format!("Close {}", tab.title)),
                colors.dimmed,
                colors.hover,
                move |_, cx| {
                    close.update(cx, |root, cx| {
                        root.workspace.close(pane, ti);
                        cx.notify();
                    });
                },
            )),
        );
    }

    // Split button: opens a terminal in a new pane to the right.
    let split = handle.clone();
    bar = bar.child(div().ml_auto().child(icon_button(
        SharedString::from(format!("split-{pane}")),
        "plus",
        "Split terminal right",
        colors.dimmed,
        colors.hover,
        move |window, cx| {
            split.update(cx, |root, cx| {
                root.workspace.active_pane = pane;
                root.split_terminal_pane(window, cx);
                cx.notify();
            });
        },
    )));
    bar
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
                compose.clone(),
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
                self.integration_error.clone(),
                handle,
                window,
                cx,
            )
            .into_any_element(),
            TabKind::Accounts => {
                accounts::accounts_view(self.accounts(), handle, window, cx).into_any_element()
            }
            TabKind::Inbox => notifications::inbox_view(self.notifications(), handle, window, cx)
                .into_any_element(),
            TabKind::Plugins => {
                crate::plugins::plugins_view(self.plugins(), self.plugins_dir(), window, cx)
                    .into_any_element()
            }
            TabKind::Settings => match self.settings_inputs.clone() {
                Some(inputs) => crate::settings::settings_view(
                    self.settings.clone(),
                    self.settings_diagnostics.clone(),
                    self.agent_reports(),
                    inputs,
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
                s = s.item("Run fan-out", move |window, cx| {
                    h.update(cx, |root, cx| {
                        root.run_fanout(window, cx);
                        cx.notify();
                    });
                });
                let h = handle.clone();
                s = s.item("Run checks", move |_, cx| {
                    h.update(cx, |root, cx| {
                        root.run_checks(cx);
                        cx.notify();
                    });
                });
                let h = handle.clone();
                s = s.item("Open selected run terminal", move |_, cx| {
                    h.update(cx, |root, cx| {
                        if let Some(id) = root.current_run_id() {
                            root.open_run_terminal(id);
                            cx.notify();
                        } else {
                            root.push_error("No run selected", "Select a run first.");
                        }
                    });
                });
                let h = handle.clone();
                s = s.item("Cancel selected run", move |_, cx| {
                    h.update(cx, |root, cx| {
                        if let Some(id) = root.current_run_id() {
                            root.cancel_run(id, cx);
                            cx.notify();
                        } else {
                            root.push_error("No run selected", "Select a run first.");
                        }
                    });
                });
                let h = handle.clone();
                s = s.item("Retry selected run", move |window, cx| {
                    h.update(cx, |root, cx| {
                        if let Some(id) = root.current_run_id() {
                            root.retry_run(id, window, cx);
                            cx.notify();
                        } else {
                            root.push_error("No run selected", "Select a run first.");
                        }
                    });
                });
                let h = handle.clone();
                s = s.item("Merge selected run", move |_, cx| {
                    h.update(cx, |root, cx| {
                        if let Some(id) = root.current_run_id() {
                            root.request_merge(id);
                            cx.notify();
                        } else {
                            root.push_error("No run selected", "Select a run first.");
                        }
                    });
                });
                let h = handle.clone();
                s = s.item("Open Settings", move |window, cx| {
                    h.update(cx, |root, cx| {
                        root.open_view(View::Settings, window, cx);
                        cx.notify();
                    });
                });
                let h = handle.clone();
                s = s.item("Open settings.json", move |_, cx| {
                    if let Err(error) = crate::menus::open_settings_file(cx) {
                        h.update(cx, |root, cx| {
                            root.push_error("Could not open settings", error);
                            cx.notify();
                        });
                    }
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
    let palette_btn = icon_button(
        "tb-palette",
        "command",
        "Command palette",
        colors.text,
        colors.hover,
        move |window, cx| palette.update(cx, |spotlight, cx| spotlight.open(window, cx)),
    );
    let search_btn = icon_button(
        "tb-quickopen",
        "search",
        "Quick open",
        colors.text,
        colors.hover,
        move |window, cx| quickopen.update(cx, |spotlight, cx| spotlight.open(window, cx)),
    );
    let open_btn = icon_button(
        "tb-open",
        "folder",
        "Open repository",
        colors.text,
        colors.hover,
        move |_, cx| open_project(handle.clone(), cx),
    );
    let theme_btn = icon_button(
        "tb-theme",
        "sun",
        "Toggle theme",
        colors.text,
        colors.hover,
        move |_, cx| theme::toggle(cx),
    );

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
                .child(palette_btn)
                .child(search_btn)
                .child(open_btn)
                .child(theme_btn),
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
