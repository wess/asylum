//! The navbar: a styled view menu (Lucide icons) over an expandable
//! project → task → run workspace tree.

use gpui::prelude::*;
use gpui::{div, px, App, Entity, Hsla, IntoElement, MouseButton, SharedString, Window};
use guise::prelude::*;

use crate::icons::{icon, icon_button};
use crate::menu;
use crate::state::{Root, TreeProject, TreeRun, TreeTask, View};
use store::{RunStatus, TaskStatus};

/// Palette pulled from the active guise theme (adapts to light/dark).
struct Palette {
    text: Hsla,
    dimmed: Hsla,
    primary: Hsla,
    hover: Hsla,
    border: Hsla,
}

fn palette(cx: &App) -> Palette {
    let t = guise::theme::theme(cx);
    Palette {
        text: t.text().hsla(),
        dimmed: t.dimmed().hsla(),
        primary: t.primary().hsla(),
        hover: t.surface_hover().hsla(),
        border: t.border().hsla(),
    }
}

/// The whole navbar: the view menu on top, the workspace tree below.
#[allow(clippy::too_many_arguments)]
pub fn navbar(
    active_view: Option<View>,
    unread: usize,
    tree: Vec<TreeProject>,
    project_id: Option<i64>,
    task_id: Option<i64>,
    collapsed: bool,
    handle: Entity<Root>,
    _window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let p = palette(cx);
    let mut nav = div()
        .flex()
        .flex_col()
        .w_full()
        .h_full()
        .child(collapse_control(collapsed, handle.clone(), &p))
        .child(nav_menu(active_view, unread, collapsed, handle.clone(), &p));
    if !collapsed {
        nav = nav
            .child(div().w_full().h(px(1.0)).bg(p.border))
            .child(workspace_tree(tree, project_id, task_id, handle, &p));
    }
    nav
}

fn collapse_control(collapsed: bool, handle: Entity<Root>, p: &Palette) -> impl IntoElement {
    let label = if collapsed {
        "Expand sidebar"
    } else {
        "Collapse sidebar"
    };
    div()
        .flex()
        .items_center()
        .h(px(36.0))
        .px(px(8.0))
        .border_b_1()
        .border_color(p.border)
        .when(collapsed, |element| element.justify_center())
        .when(!collapsed, |element| element.justify_end())
        .child(icon_button(
            "sidebar-toggle",
            if collapsed {
                "panelleftopen"
            } else {
                "panelleftclose"
            },
            label,
            p.dimmed,
            p.hover,
            move |_, cx| {
                handle.update(cx, |root, cx| {
                    root.sidebar_collapsed = !root.sidebar_collapsed;
                    cx.notify();
                });
            },
        ))
}

/// The vertical view switcher. Clicking an item opens (or focuses) its tab.
fn nav_menu(
    active_view: Option<View>,
    unread: usize,
    collapsed: bool,
    handle: Entity<Root>,
    p: &Palette,
) -> impl IntoElement {
    let mut col = div()
        .flex()
        .flex_col()
        .gap_1()
        .when(collapsed, |element| element.p(px(6.0)))
        .when(!collapsed, |element| element.p(px(8.0)));
    for (v, _glyph, _label) in View::BAR {
        let v = *v;
        let active = Some(v) == active_view;
        let h = handle.clone();
        let icon_color = if active { p.primary } else { p.dimmed };
        let text_color = if active { p.text } else { p.dimmed };

        let mut row = div()
            .id(SharedString::from(format!("nav-{}", v.label())))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .px(px(10.0))
            .py(px(7.0))
            .rounded(px(7.0))
            .cursor_pointer()
            .tab_index(0)
            .role(gpui::accesskit::Role::Button)
            .aria_label(v.label())
            .aria_selected(active)
            .tooltip(guise::tooltip(v.label()))
            .focus_visible(move |style| style.border_1().border_color(p.primary))
            .when(collapsed, |element| {
                element.justify_center().gap(px(4.0)).px(px(4.0))
            })
            .child(icon(v.icon(), 16.0).text_color(icon_color));
        if !collapsed {
            row = row.child(
                div()
                    .flex_1()
                    .text_color(text_color)
                    .text_size(px(13.0))
                    .child(SharedString::from(v.label())),
            );
        }
        if active {
            row = row.bg(p.hover);
        }
        if v == View::Notifications && unread > 0 {
            row = row.child(
                Badge::new(SharedString::from(unread.to_string()))
                    .color(ColorName::Red)
                    .variant(Variant::Filled),
            );
        }
        col = col.child(row.on_click(move |_, window, cx| {
            h.update(cx, |root, cx| {
                root.open_view(v, window, cx);
                cx.notify();
            });
        }));
    }
    col
}

/// The expandable project → task → run tree.
fn workspace_tree(
    tree: Vec<TreeProject>,
    project_id: Option<i64>,
    task_id: Option<i64>,
    handle: Entity<Root>,
    p: &Palette,
) -> impl IntoElement {
    let mut col = div()
        .flex()
        .flex_col()
        .gap(px(1.0))
        .px(px(6.0))
        .py(px(8.0))
        .overflow_hidden();
    col = col.child(
        div()
            .px(px(8.0))
            .py(px(4.0))
            .text_color(p.dimmed)
            .text_size(px(11.0))
            .child("WORKSPACE"),
    );

    for proj in tree {
        col = col.child(project_row(&proj, project_id, handle.clone(), p));
        if proj.expanded {
            for task in &proj.tasks {
                col = col.child(task_row(task, task_id, handle.clone(), p));
                if task.expanded {
                    for run in &task.runs {
                        col = col.child(run_row(run, handle.clone(), p));
                    }
                }
            }
        }
    }
    col
}

fn project_row(
    proj: &TreeProject,
    project_id: Option<i64>,
    handle: Entity<Root>,
    p: &Palette,
) -> impl IntoElement {
    let id = proj.id;
    let selected = Some(id) == project_id;
    let chevron = if proj.expanded {
        "chevron-down"
    } else {
        "chevron-right"
    };

    let toggle = handle.clone();
    let select = handle.clone();
    let pin = handle.clone();
    let ctx = handle.clone();

    let mut row = div()
        .id(SharedString::from(format!("prow-{id}")))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .px(px(4.0))
        .py(px(5.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .role(gpui::accesskit::Role::TreeItem)
        .aria_label(SharedString::from(proj.name.clone()))
        .aria_selected(selected)
        .aria_expanded(proj.expanded)
        .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
            menu::project(ctx.clone(), id, ev.position, window, cx);
        });
    if selected {
        row = row.bg(p.hover);
    }
    row.child(
        // Chevron - toggles expand.
        div()
            .id(SharedString::from(format!("chev-p-{id}")))
            .p(px(2.0))
            .cursor_pointer()
            .tab_index(0)
            .role(gpui::accesskit::Role::Button)
            .aria_label(if proj.expanded {
                "Collapse project"
            } else {
                "Expand project"
            })
            .focus_visible(move |style| style.border_1().border_color(p.primary))
            .child(icon(chevron, 14.0).text_color(p.dimmed))
            .on_click(move |_, _, cx| {
                toggle.update(cx, |root, cx| {
                    root.toggle_expanded(&format!("project-{id}"));
                    cx.notify();
                });
            }),
    )
    .child(icon("folder", 15.0).text_color(p.primary))
    .child(
        // Name - selects the project.
        div()
            .id(SharedString::from(format!("name-p-{id}")))
            .flex_1()
            .text_color(p.text)
            .text_size(px(13.0))
            .tab_index(0)
            .role(gpui::accesskit::Role::Button)
            .aria_label(SharedString::from(format!("Open project {}", proj.name)))
            .focus_visible(move |style| style.border_1().border_color(p.primary))
            .child(SharedString::from(proj.name.clone()))
            .on_click(move |_, window, cx| {
                select.update(cx, |root, cx| {
                    root.select_project(id);
                    root.open_view(View::Tasks, window, cx);
                    cx.notify();
                });
            }),
    )
    .child(
        // Pin star.
        div()
            .id(SharedString::from(format!("pin-p-{id}")))
            .p(px(2.0))
            .cursor_pointer()
            .tab_index(0)
            .role(gpui::accesskit::Role::Button)
            .aria_label(if proj.pinned {
                "Unpin project"
            } else {
                "Pin project"
            })
            .focus_visible(move |style| style.border_1().border_color(p.primary))
            .child(icon("star", 13.0).text_color(if proj.pinned { p.primary } else { p.dimmed }))
            .on_click(move |_, _, cx| {
                pin.update(cx, |root, cx| {
                    root.toggle_pin(id);
                    cx.notify();
                });
            }),
    )
}

fn task_row(
    task: &TreeTask,
    task_id: Option<i64>,
    handle: Entity<Root>,
    p: &Palette,
) -> impl IntoElement {
    let id = task.id;
    let selected = Some(id) == task_id;
    let chevron = if task.expanded {
        "chevron-down"
    } else {
        "chevron-right"
    };
    let (status_icon, status_color) = task_status_icon(task.status, p);

    let toggle = handle.clone();
    let select = handle.clone();
    let ctx = handle.clone();

    let mut row = div()
        .id(SharedString::from(format!("trow-{id}")))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .pl(px(18.0))
        .pr(px(4.0))
        .py(px(4.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .role(gpui::accesskit::Role::TreeItem)
        .aria_label(SharedString::from(task.title.clone()))
        .aria_selected(selected)
        .aria_expanded(task.expanded)
        .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
            menu::task(ctx.clone(), id, ev.position, window, cx);
        });
    if selected {
        row = row.bg(p.hover);
    }
    row.child(
        div()
            .id(SharedString::from(format!("chev-t-{id}")))
            .p(px(2.0))
            .cursor_pointer()
            .tab_index(0)
            .role(gpui::accesskit::Role::Button)
            .aria_label(if task.expanded {
                "Collapse task"
            } else {
                "Expand task"
            })
            .focus_visible(move |style| style.border_1().border_color(p.primary))
            .child(icon(chevron, 13.0).text_color(p.dimmed))
            .on_click(move |_, _, cx| {
                toggle.update(cx, |root, cx| {
                    root.toggle_expanded(&format!("task-{id}"));
                    cx.notify();
                });
            }),
    )
    .child(icon(status_icon, 13.0).text_color(status_color))
    .child(
        div()
            .id(SharedString::from(format!("name-t-{id}")))
            .flex_1()
            .text_color(if selected { p.text } else { p.dimmed })
            .text_size(px(12.5))
            .tab_index(0)
            .role(gpui::accesskit::Role::Button)
            .aria_label(SharedString::from(format!("Open task {}", task.title)))
            .focus_visible(move |style| style.border_1().border_color(p.primary))
            .child(SharedString::from(task.title.clone()))
            .on_click(move |_, window, cx| {
                select.update(cx, |root, cx| {
                    root.task_id = Some(id);
                    root.selected_run_id = root
                        .db
                        .runs(id)
                        .ok()
                        .and_then(|runs| runs.first().map(|run| run.id));
                    root.open_view(View::Tasks, window, cx);
                    cx.notify();
                });
            }),
    )
}

fn run_row(run: &TreeRun, handle: Entity<Root>, p: &Palette) -> impl IntoElement {
    let (status_icon, status_color) = run_status_icon(run.status, p);
    let name = agent::find(&run.agent)
        .map(|a| a.name)
        .unwrap_or(run.agent.as_str());
    let id = run.id;
    let select = handle.clone();
    div()
        .id(SharedString::from(format!("tree-run-{}", run.id)))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .pl(px(40.0))
        .py(px(3.0))
        .cursor_pointer()
        .tab_index(0)
        .role(gpui::accesskit::Role::Button)
        .aria_label(SharedString::from(format!("Open {name} run")))
        .focus_visible(move |style| style.border_1().border_color(p.primary))
        .on_mouse_down(MouseButton::Right, move |ev, window, cx| {
            menu::run(handle.clone(), id, ev.position, window, cx);
        })
        .on_click(move |_, window, cx| {
            select.update(cx, |root, cx| {
                root.select_run(id);
                root.open_view(View::Tasks, window, cx);
                cx.notify();
            });
        })
        .child(icon(status_icon, 12.0).text_color(status_color))
        .child(
            div()
                .text_color(p.dimmed)
                .text_size(px(12.0))
                .child(SharedString::from(name.to_string())),
        )
}

fn task_status_icon(status: TaskStatus, p: &Palette) -> (&'static str, Hsla) {
    match status {
        TaskStatus::Draft => ("circle", p.dimmed),
        TaskStatus::Running => ("loader", p.primary),
        TaskStatus::Review => ("eye", p.primary),
        TaskStatus::Merged => ("circle-check", p.primary),
        TaskStatus::Archived => ("circle", p.dimmed),
    }
}

fn run_status_icon(status: RunStatus, p: &Palette) -> (&'static str, Hsla) {
    match status {
        RunStatus::Queued => ("circle", p.dimmed),
        RunStatus::Running => ("loader", p.primary),
        RunStatus::Succeeded => ("circle-check", p.primary),
        RunStatus::Failed => ("circle-x", p.dimmed),
        RunStatus::Cancelled => ("circle", p.dimmed),
    }
}
