//! Window chrome around the main area: the titlebar header, status footer,
//! onboarding screens, notice stack, confirm bar, and sidebar resizer.

use gpui::prelude::*;
use gpui::{
    div, px, App, Empty, Entity, IntoElement, MouseButton, PathPromptOptions, SharedString, Window,
    WindowControlArea,
};
use guise::prelude::*;

use super::{Colors, SidebarDrag, FOOTER_HEIGHT, HEADER_HEIGHT, TRAFFIC_LIGHT_INSET};
use crate::control::Button;
use crate::icons::{icon, icon_button};
use crate::state::Root;
use crate::theme;

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
pub(super) fn onboarding(
    handle: Entity<Root>,
    pending: Option<std::path::PathBuf>,
    reports: Vec<(agent::registry::Agent, agent::doctor::Report)>,
    notices: Vec<crate::run::Notice>,
    primary: gpui::Hsla,
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
        .gap_4()
        .p(px(32.0));
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
            .child(icon("git-branch", 40.0).text_color(primary))
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
    let readiness = if verified > 0 {
        format!("Ready to work · {verified} agent(s) verified")
    } else if installed > 0 {
        format!("{installed} agent(s) found · verify after opening a project")
    } else {
        "No agent CLI found · configure one before your first run".to_string()
    };
    body.child(
        div()
            .w_full()
            .max_w(px(860.0))
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(48.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(320.0))
                    .gap_3()
                    .child(icon("git-branch", 40.0).text_color(primary))
                    .child(Title::new("Run the same task. Compare the evidence.").order(1))
                    .child(
                        Text::new("Asylum gives each coding agent an isolated copy of your repository, then brings their changes, checks, and output together for review.")
                            .size(Size::Sm)
                            .dimmed(),
                    )
                    .child(
                        Badge::new(SharedString::from(readiness))
                            .color(if verified > 0 { ColorName::Green } else { ColorName::Orange })
                            .variant(Variant::Light),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                Button::new("open-project", "Open a repository…")
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
                                            // Deep-link straight into the Agents
                                            // section instead of leaving the user to
                                            // find and expand it themselves.
                                            root.settings_collapsed =
                                                crate::settings::collapsed_except("agents");
                                            root.onboarding_settings = true;
                                            cx.notify();
                                        });
                                    }),
                            ),
                    )
                    .child(
                        Text::new("Your repository is not modified when you open it. Changes happen in separate git worktrees until you choose to merge.")
                            .size(Size::Xs)
                            .dimmed(),
                    ),
            )
            .child(onboarding_path()),
    )
}

fn onboarding_path() -> impl IntoElement {
    let mut path = div()
        .flex()
        .flex_col()
        .min_w(px(260.0))
        .gap_3()
        .p_4()
        .border_1()
        .rounded(px(8.0))
        .child(Text::new("Your first run").bold());
    for (number, title, detail) in [
        ("1", "Open a repository", "Choose an existing git project."),
        (
            "2",
            "Describe one outcome",
            "Use a template or write a focused task.",
        ),
        (
            "3",
            "Start with one agent",
            "Add more agents when comparison helps.",
        ),
        (
            "4",
            "Review before merging",
            "Run checks and inspect the diff.",
        ),
    ] {
        path = path.child(
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap_2()
                .child(
                    Badge::new(number)
                        .color(ColorName::Blue)
                        .variant(Variant::Light),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Text::new(title).size(Size::Sm).bold())
                        .child(Text::new(detail).size(Size::Xs).dimmed()),
                ),
        );
    }
    path
}

#[allow(clippy::too_many_arguments)]
pub(super) fn onboarding_settings(
    settings: config::Settings,
    diagnostics: Vec<config::Diagnostic>,
    agents: Vec<crate::settings::AgentRow>,
    inputs: crate::settings::Inputs,
    collapsed: std::collections::HashSet<&'static str>,
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
            agents,
            inputs,
            collapsed,
            handle,
            window,
            cx,
        ))
        .into_any_element()
}

pub(super) fn notice_stack(
    notices: Vec<crate::run::Notice>,
    handle: Entity<Root>,
) -> impl IntoElement {
    // Anchored below the header's real height (plus a small gap) rather than
    // an independent magic number, so it can never drift into overlapping
    // the header's action cluster.
    let mut stack = div()
        .absolute()
        .top(px(HEADER_HEIGHT + 8.0))
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

pub(super) fn confirm_bar(
    action: crate::run::ConfirmAction,
    sidebar_extent: f32,
    colors: &Colors,
    handle: Entity<Root>,
) -> impl IntoElement {
    let confirm = handle.clone();
    let cancel = handle;
    div()
        .absolute()
        .bottom(px(FOOTER_HEIGHT + 10.0))
        // Anchored to the sidebar's real width (collapsed or expanded)
        // instead of a fixed offset, so a resize never leaves it stranded
        // over the navbar or detached from it.
        .left(px(sidebar_extent + 12.0))
        .right(px(12.0))
        .p_3()
        .border_1()
        .border_color(colors.border)
        .rounded(px(6.0))
        .bg(colors.surface)
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

/// The draggable divider on the expanded navbar's right edge. Sits between the
/// header and footer at `x = width`; dragging it is followed by the root
/// wrapper's `on_drag_move`, which updates `Root::sidebar_width`.
pub(super) fn sidebar_resizer(width: f32) -> impl IntoElement {
    div()
        .id("sidebar-resizer")
        .absolute()
        .top(px(HEADER_HEIGHT))
        .bottom(px(FOOTER_HEIGHT))
        .left(px(width - 3.0))
        .w(px(6.0))
        .cursor_col_resize()
        .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 1.0, 0.12)))
        .on_drag(SidebarDrag, |_, _, _, cx| cx.new(|_| Empty))
}

/// The titlebar. With a transparent native titlebar, this *is* the window's top
/// chrome: the macOS traffic lights float into the left inset, the brand sits
/// beside them, a draggable filler moves the window (double-click zooms), and
/// the actions live on the right.
pub(super) fn header(
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
pub(super) fn footer(counts: (usize, usize, usize), unread: usize) -> impl IntoElement {
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
