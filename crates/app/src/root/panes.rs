//! Pane and tab-strip rendering for the splittable main area.

use gpui::prelude::*;
use gpui::{div, px, Entity, IntoElement, SharedString};

use super::Colors;
use crate::icons::{icon, icon_button};
use crate::state::Root;

/// A render-time snapshot of a pane (owned; the workspace borrow is released).
pub(super) struct PaneSnap {
    pub(super) index: usize,
    pub(super) active: bool,
    pub(super) kind: Option<crate::workspace::TabKind>,
    pub(super) tabs: Vec<TabSnap>,
}

pub(super) struct TabSnap {
    pub(super) index: usize,
    pub(super) id: u64,
    pub(super) title: String,
    pub(super) icon: &'static str,
    pub(super) active: bool,
}

/// One pane: a tab bar over the active tab's content.
pub(super) fn pane_view(
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
