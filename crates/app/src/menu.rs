//! Right-click context menus for the workspace tree.
//!
//! Each builder rebuilds a fresh [`guise::overlay::ContextMenu`] with the actions
//! for its target, shows it at the click position, and stashes it on [`Root`] so
//! the root view overlays it. Item handlers act on the app via the captured
//! [`Entity<Root>`].

use gpui::prelude::*;
use gpui::{App, Entity, Pixels, Point, Window};
use guise::overlay::ContextMenu;

use crate::state::{Root, View};

/// The context menu for a project row.
pub fn project(
    handle: Entity<Root>,
    id: i64,
    pos: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let h = handle.clone();
    handle.update(cx, |root, cx| {
        let pinned = root.db.project(id).map(|p| p.pinned).unwrap_or(false);
        let path = root.project_path_of(id);
        let menu = cx.new(|cx| {
            let mut m = ContextMenu::new(cx).section("Project");

            let a = h.clone();
            m = m.item("New task", move |window, cx| {
                a.update(cx, |r, c| {
                    r.select_project(id);
                    r.open_view(View::Tasks, window, c);
                    c.notify();
                });
            });

            let a = h.clone();
            m = m.item("Open editor", move |window, cx| {
                a.update(cx, |r, c| {
                    r.select_project(id);
                    r.open_view(View::Editor, window, c);
                    c.notify();
                });
            });

            let a = h.clone();
            m = m.item("Open terminal", move |window, cx| {
                a.update(cx, |r, c| {
                    r.select_project(id);
                    r.open_view(View::Terminal, window, c);
                    c.notify();
                });
            });

            let a = h.clone();
            m = m.item(if pinned { "Unpin" } else { "Pin" }, move |_, cx| {
                a.update(cx, |r, c| {
                    r.toggle_pin(id);
                    c.notify();
                });
            });

            if let Some(p) = path {
                m = m.item("Reveal in Finder", move |_, cx| {
                    cx.reveal_path(std::path::Path::new(&p));
                });
            }

            m = m.divider();
            let a = h.clone();
            m = m.danger_item("Remove project", move |_, cx| {
                a.update(cx, |r, c| {
                    r.confirm = Some(crate::run::ConfirmAction::DeleteProject(id));
                    c.notify();
                });
            });
            m
        });
        menu.update(cx, |m, cx| m.show(pos, window, cx));
        root.context_menu = Some(menu);
        cx.notify();
    });
}

/// The context menu for a task row.
pub fn task(handle: Entity<Root>, id: i64, pos: Point<Pixels>, window: &mut Window, cx: &mut App) {
    let h = handle.clone();
    handle.update(cx, |root, cx| {
        let menu = cx.new(|cx| {
            let mut m = ContextMenu::new(cx).section("Task");

            let a = h.clone();
            m = m.item("Fan out to agents", move |window, cx| {
                a.update(cx, |r, c| {
                    r.task_id = Some(id);
                    r.run_fanout(window, c);
                    r.open_view(View::Tasks, window, c);
                    c.notify();
                });
            });

            let a = h.clone();
            m = m.item("Review diff", move |window, cx| {
                a.update(cx, |r, c| {
                    r.task_id = Some(id);
                    r.open_view(View::Diff, window, c);
                    c.notify();
                });
            });

            let a = h.clone();
            m = m.item("Archive", move |_, cx| {
                a.update(cx, |r, c| {
                    r.confirm = Some(crate::run::ConfirmAction::ArchiveTask(id));
                    c.notify();
                });
            });

            m = m.divider();
            let a = h.clone();
            m = m.danger_item("Delete task", move |_, cx| {
                a.update(cx, |r, c| {
                    r.confirm = Some(crate::run::ConfirmAction::DeleteTask(id));
                    c.notify();
                });
            });
            m
        });
        menu.update(cx, |m, cx| m.show(pos, window, cx));
        root.context_menu = Some(menu);
        cx.notify();
    });
}

/// The context menu for a run row.
pub fn run(handle: Entity<Root>, id: i64, pos: Point<Pixels>, window: &mut Window, cx: &mut App) {
    let h = handle.clone();
    handle.update(cx, |root, cx| {
        let status = root.db.run(id).ok().map(|run| run.status);
        let menu = cx.new(|cx| {
            let mut m = ContextMenu::new(cx).section("Run");

            let a = h.clone();
            m = m.item("Open terminal", move |window, cx| {
                a.update(cx, |r, c| {
                    r.open_run_terminal(id);
                    let _ = window;
                    c.notify();
                });
            });

            let a = h.clone();
            m = m.item("Review diff", move |window, cx| {
                a.update(cx, |r, c| {
                    r.select_run(id);
                    r.open_view(View::Diff, window, c);
                    c.notify();
                });
            });

            if status == Some(store::RunStatus::Succeeded) {
                let a = h.clone();
                m = m.item("Merge winner", move |_, cx| {
                    a.update(cx, |r, c| {
                        r.request_merge(id);
                        c.notify();
                    });
                });

                let a = h.clone();
                m = m.item("Create PR", move |_, cx| {
                    a.update(cx, |r, c| {
                        r.create_pr_for_run(id);
                        c.notify();
                    });
                });
            }

            m = m.divider();
            if status.is_some_and(|status| {
                matches!(status, store::RunStatus::Queued | store::RunStatus::Running)
            }) {
                let a = h.clone();
                m = m.danger_item("Cancel run", move |_, cx| {
                    a.update(cx, |r, c| {
                        r.cancel_run(id, c);
                        c.notify();
                    });
                });
            } else if status.is_some_and(store::RunStatus::is_terminal) {
                let a = h.clone();
                m = m.item("Retry run", move |window, cx| {
                    a.update(cx, |r, c| {
                        r.retry_run(id, window, c);
                        c.notify();
                    });
                });
            }
            m
        });
        menu.update(cx, |m, cx| m.show(pos, window, cx));
        root.context_menu = Some(menu);
        cx.notify();
    });
}
