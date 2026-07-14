use std::rc::Rc;

use gpui::prelude::*;
use gpui::{div, px, App, ElementId, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::icons::icon;
use crate::state::Root;

use super::super::Mode;
use super::Palette;

pub(super) fn pane(
    root: &Root,
    handle: Entity<Root>,
    compact: bool,
    palette: Palette,
) -> impl IntoElement {
    let input = root.note.title.clone().expect("note title");
    let editor = root.note.editor.clone().expect("note editor");
    let preview = root.note.preview.clone().expect("note preview");
    let selected = root.current_note().is_some();
    let editmode = handle.clone();
    let previewmode = handle.clone();
    let splitmode = handle.clone();

    let modes = div()
        .flex()
        .flex_row()
        .gap(px(1.0))
        .p(px(2.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(palette.border)
        .child(modebutton(
            "note-edit",
            "Edit",
            "file-pen",
            Mode::Edit,
            root.note.view,
            editmode,
            palette,
        ))
        .child(modebutton(
            "note-preview",
            "Preview",
            "eye",
            Mode::Preview,
            root.note.view,
            previewmode,
            palette,
        ))
        .child(modebutton(
            "note-split",
            "Split",
            "git-compare",
            Mode::Split,
            root.note.view,
            splitmode,
            palette,
        ));

    let task = handle.clone();
    let attach = handle.clone();
    let send = handle.clone();
    let actions = div()
        .flex()
        .flex_row()
        .gap_1()
        .child(commandbutton(
            "note-task",
            "list-todo",
            "Create task from note",
            selected,
            palette,
            move |_, cx| {
                task.update(cx, |root, cx| {
                    root.create_task_from_note();
                    cx.notify();
                });
            },
        ))
        .child(commandbutton(
            "note-attach",
            "git-branch",
            "Attach note to selected run",
            selected,
            palette,
            move |_, cx| {
                attach.update(cx, |root, cx| {
                    root.attach_note_to_selected_run();
                    cx.notify();
                });
            },
        ))
        .child(commandbutton(
            "note-send",
            "play",
            "Send selection to agent",
            selected,
            palette,
            move |window, cx| {
                send.update(cx, |root, cx| root.send_note_selection(window, cx));
            },
        ));

    let rename = handle.clone();
    let delete = handle.clone();
    let mut titlebar = div().flex().flex_row().items_center().gap_1();
    if !compact && !root.note.files_open {
        let show = handle.clone();
        titlebar = titlebar.child(commandbutton(
            "show-note-files",
            "panelleftopen",
            "Show files",
            true,
            palette,
            move |_, cx| {
                show.update(cx, |root, cx| {
                    root.note.files_open = true;
                    cx.notify();
                });
            },
        ));
    }
    titlebar = titlebar
        .child(div().flex_1().min_w_0().child(input))
        .child(
            Text::new(if root.note.saved { "Saved" } else { "Saving" })
                .size(Size::Xs)
                .dimmed(),
        )
        .child(commandbutton(
            "rename-note",
            "file-pen",
            "Rename note",
            selected,
            palette,
            move |_, cx| {
                rename.update(cx, |root, cx| root.rename_current_note(cx));
            },
        ))
        .child(commandbutton(
            "delete-note",
            "circle-x",
            "Delete note",
            selected,
            palette,
            move |_, cx| {
                delete.update(cx, |root, cx| {
                    root.request_delete_note();
                    cx.notify();
                });
            },
        ));
    if !compact && !root.note.details_open {
        let show = handle.clone();
        titlebar = titlebar.child(commandbutton(
            "show-note-properties",
            "settings",
            "Show properties",
            true,
            palette,
            move |_, cx| {
                show.update(cx, |root, cx| {
                    root.note.details_open = true;
                    cx.notify();
                });
            },
        ));
    }

    let head = div()
        .flex()
        .flex_col()
        .px(px(12.0))
        .py(px(6.0))
        .border_b_1()
        .border_color(palette.border)
        .child(titlebar)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .mt(px(4.0))
                .child(modes)
                .child(div().ml_auto().child(actions)),
        );

    let content = match root.note.view {
        Mode::Edit => editor.into_any_element(),
        Mode::Preview => preview.into_any_element(),
        Mode::Split => div()
            .flex()
            .flex_row()
            .size_full()
            .child(div().flex_1().min_w_0().border_r_1().child(editor))
            .child(div().flex_1().min_w_0().child(preview))
            .into_any_element(),
    };
    let mut body = div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .overflow_hidden()
        .child(head)
        .child(div().flex_1().min_h_0().child(content));
    if !root.note.suggestions.is_empty() {
        let mut popup = div()
            .absolute()
            .top(px(76.0))
            .left(px(12.0))
            .w(px(280.0))
            .p(px(5.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(palette.border)
            .bg(palette.surface);
        for suggestion in &root.note.suggestions {
            let title = suggestion.title.clone();
            let insert = handle.clone();
            popup = popup.child(
                div()
                    .id(SharedString::from(format!("suggest-{}", suggestion.path)))
                    .px(px(8.0))
                    .py(px(5.0))
                    .cursor_pointer()
                    .tab_index(0)
                    .role(gpui::accesskit::Role::Button)
                    .aria_label(SharedString::from(format!(
                        "Link note {}",
                        suggestion.title
                    )))
                    .on_click(move |_, window, cx| {
                        insert.update(cx, |root, cx| root.insert_note_link(&title, window, cx));
                    })
                    .child(Text::new(SharedString::from(suggestion.title.clone())).size(Size::Sm)),
            );
        }
        body = body.child(popup);
    }
    body.when(!compact, |element| element.flex_1())
}

fn modebutton(
    id: &'static str,
    label: &'static str,
    iconname: &'static str,
    mode: Mode,
    current: Mode,
    handle: Entity<Root>,
    palette: Palette,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(28.0))
        .h(px(24.0))
        .rounded(px(4.0))
        .cursor_pointer()
        .tab_index(0)
        .role(gpui::accesskit::Role::Tab)
        .aria_label(label)
        .aria_selected(mode == current)
        .tooltip(guise::tooltip(label))
        .focus_visible(move |style| style.border_1().border_color(palette.primary))
        .hover(move |style| style.bg(palette.hover))
        .when(mode == current, |element| element.bg(palette.hover))
        .child(icon(iconname, 14.0).text_color(if mode == current {
            palette.text
        } else {
            palette.dimmed
        }))
        .on_click(move |_, _, cx| {
            handle.update(cx, |root, cx| {
                root.set_note_view(mode, cx);
                cx.notify();
            });
        })
}

fn commandbutton(
    id: impl Into<ElementId>,
    iconname: &'static str,
    label: impl Into<SharedString>,
    enabled: bool,
    palette: Palette,
    activate: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = label.into();
    let activate = Rc::new(activate);
    let mut button = div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(28.0))
        .h(px(24.0))
        .rounded(px(5.0))
        .role(gpui::accesskit::Role::Button)
        .aria_label(label.clone())
        .tooltip(guise::tooltip(label))
        .child(icon(iconname, 15.0).text_color(palette.dimmed));
    if enabled {
        button = button
            .cursor_pointer()
            .tab_index(0)
            .hover(move |style| style.bg(palette.hover))
            .focus_visible(move |style| style.border_1().border_color(palette.primary))
            .on_click(move |_, window, cx| activate(window, cx));
    } else {
        button = button.opacity(0.4);
    }
    button
}
