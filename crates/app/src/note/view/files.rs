use gpui::prelude::*;
use gpui::{div, px, Entity, IntoElement, SharedString};
use guise::prelude::*;

use crate::control::Button;
use crate::icons::{icon, icon_button};
use crate::state::Root;

use super::Palette;

pub(super) fn pane(
    root: &Root,
    handle: Entity<Root>,
    compact: bool,
    palette: Palette,
) -> impl IntoElement {
    let notes = root.filtered_notes();
    let count = notes.len();
    let search = root.note.search.clone().expect("note search");
    let mut pane = div()
        .flex()
        .flex_col()
        .h_full()
        .min_h_0()
        .text_color(palette.text)
        .when(!compact, |element| {
            element
                .w(px(184.0))
                .border_r_1()
                .border_color(palette.border)
                .flex_none()
        });

    let create = handle.clone();
    let mut header = div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(36.0))
        .px(px(8.0))
        .gap_1()
        .child(Text::new("Files").size(Size::Xs).bold())
        .child(Text::new(count.to_string()).size(Size::Xs).dimmed())
        .child(div().ml_auto().child(icon_button(
            "create-blank-note",
            "plus",
            "New note",
            palette.dimmed,
            palette.hover,
            move |_, cx| {
                create.update(cx, |root, cx| {
                    root.create_note(notes::Template::Blank, cx);
                    cx.notify();
                });
            },
        )));
    if !compact {
        let collapse = handle.clone();
        header = header.child(icon_button(
            "collapse-note-files",
            "panelleftclose",
            "Hide files",
            palette.dimmed,
            palette.hover,
            move |_, cx| {
                collapse.update(cx, |root, cx| {
                    root.note.files_open = false;
                    cx.notify();
                });
            },
        ));
    }
    pane = pane
        .child(header)
        .child(div().px(px(8.0)).pb(px(7.0)).child(search));

    let toggle = handle.clone();
    pane = pane.child(
        div()
            .id("note-templates-toggle")
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .mx(px(6.0))
            .px(px(6.0))
            .py(px(5.0))
            .rounded(px(5.0))
            .cursor_pointer()
            .tab_index(0)
            .role(gpui::accesskit::Role::Button)
            .aria_label("Toggle note templates")
            .aria_expanded(root.note.templates_open)
            .hover(move |style| style.bg(palette.hover))
            .on_click(move |_, _, cx| {
                toggle.update(cx, |root, cx| {
                    root.note.templates_open = !root.note.templates_open;
                    cx.notify();
                });
            })
            .child(
                icon(
                    if root.note.templates_open {
                        "chevron-down"
                    } else {
                        "chevron-right"
                    },
                    13.0,
                )
                .text_color(palette.dimmed),
            )
            .child(Text::new("Templates").size(Size::Xs).dimmed()),
    );

    let mut templates = div().flex().flex_col().gap_1().px(px(8.0)).pb(px(7.0));
    for kind in notes::Template::ALL {
        let create = handle.clone();
        templates = templates.child(
            Button::new(
                SharedString::from(format!("template-{}", kind.label())),
                kind.label(),
            )
            .size(Size::Xs)
            .variant(Variant::Subtle)
            .on_click(move |_, _, cx| {
                create.update(cx, |root, cx| {
                    root.create_note(kind, cx);
                    root.note.templates_open = false;
                    cx.notify();
                });
            }),
        );
    }
    if root.note.templates_open {
        pane = pane.child(templates);
    }

    let mut list = div()
        .id("note-files-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .px(px(6.0))
        .pb(px(8.0));
    for note in notes {
        let selected = root.note.path.as_deref() == Some(note.path.as_str());
        let path = note.path.clone();
        let select = handle.clone();
        let parent = note
            .path
            .rsplit_once('/')
            .map(|(parent, _)| parent.to_string());
        let mut row = div()
            .id(SharedString::from(format!("note-{}", note.path)))
            .flex()
            .flex_col()
            .gap_1()
            .px(px(8.0))
            .py(px(6.0))
            .rounded(px(5.0))
            .cursor_pointer()
            .tab_index(0)
            .role(gpui::accesskit::Role::Button)
            .aria_label(SharedString::from(format!("Open note {}", note.title)))
            .hover(move |style| style.bg(palette.hover))
            .when(selected, |element| element.bg(palette.hover))
            .on_click(move |_, _, cx| {
                select.update(cx, |root, cx| root.select_note(&path, cx));
            })
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(12.5))
                    .child(SharedString::from(note.title)),
            );
        if let Some(parent) = parent {
            row = row.child(
                div()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(10.5))
                    .text_color(palette.dimmed)
                    .child(SharedString::from(parent)),
            );
        }
        list = list.child(row);
    }
    pane.child(list)
}
