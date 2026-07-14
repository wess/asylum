mod details;
mod editor;
mod files;

use gpui::prelude::*;
use gpui::{div, px, App, Entity, Hsla, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::control::Button;
use crate::icons::icon_button;
use crate::state::Root;

use super::Panel;

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub text: Hsla,
    pub dimmed: Hsla,
    pub primary: Hsla,
    pub hover: Hsla,
    pub border: Hsla,
    pub surface: Hsla,
}

pub fn surface(
    root: &Root,
    handle: Entity<Root>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let compact = window.viewport_size().width < px(1050.0);
    let theme = guise::theme::theme(cx);
    let border = theme.border().hsla();
    let surface = theme.surface().hsla();
    let palette = Palette {
        text: theme.text().hsla(),
        dimmed: theme.dimmed().hsla(),
        primary: theme.primary().hsla(),
        hover: theme.surface_hover().hsla(),
        border,
        surface,
    };
    let mut page = div().flex().flex_col().size_full().overflow_hidden();
    page = page.child(toolbar(root, handle.clone(), compact, palette));
    if root.note.index.notes.is_empty() {
        return page.child(empty(handle)).into_any_element();
    }
    if compact {
        page = page.child(compacttabs(root.note.panel, handle.clone(), border));
        page = page.child(match root.note.panel {
            Panel::Files => files::pane(root, handle, true, palette).into_any_element(),
            Panel::Write => editor::pane(root, handle, true, palette).into_any_element(),
            Panel::Links => details::pane(root, handle, true, palette).into_any_element(),
        });
    } else {
        let mut workspace = div().flex().flex_row().flex_1().min_h_0();
        if root.note.files_open {
            workspace = workspace.child(files::pane(root, handle.clone(), false, palette));
        }
        workspace = workspace.child(editor::pane(root, handle.clone(), false, palette));
        if root.note.details_open {
            workspace = workspace.child(details::pane(root, handle, false, palette));
        }
        page = page.child(workspace);
    }
    page.into_any_element()
}

fn toolbar(root: &Root, handle: Entity<Root>, compact: bool, palette: Palette) -> impl IntoElement {
    let private = handle.clone();
    let repository = handle.clone();
    let refresh = handle;
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .w_full()
        .h(px(42.0))
        .px(px(12.0))
        .border_b_1()
        .border_color(palette.border);
    if !compact {
        row = row.child(Title::new("Notes").order(3));
    }
    row.child(
        div()
            .flex()
            .flex_row()
            .gap_1()
            .child(
                Button::new("vault-private", "Private")
                    .size(Size::Xs)
                    .variant(if root.note.mode == store::NoteVaultMode::Private {
                        Variant::Filled
                    } else {
                        Variant::Subtle
                    })
                    .on_click(move |_, _, cx| {
                        private.update(cx, |root, cx| {
                            root.set_note_vault_mode(store::NoteVaultMode::Private, cx);
                            cx.notify();
                        });
                    }),
            )
            .child(
                Button::new("vault-repository", "Repository")
                    .size(Size::Xs)
                    .variant(if root.note.mode == store::NoteVaultMode::Repository {
                        Variant::Filled
                    } else {
                        Variant::Subtle
                    })
                    .on_click(move |_, _, cx| {
                        repository.update(cx, |root, cx| {
                            root.set_note_vault_mode(store::NoteVaultMode::Repository, cx);
                            cx.notify();
                        });
                    }),
            ),
    )
    .child(
        Text::new(format!("{} notes", root.note.index.notes.len()))
            .size(Size::Xs)
            .dimmed(),
    )
    .child(div().ml_auto().child(icon_button(
        "refresh-notes",
        "loader",
        "Refresh notes",
        palette.dimmed,
        palette.hover,
        move |_, cx| {
            refresh.update(cx, |root, cx| root.refresh_notes(cx));
        },
    )))
}

fn compacttabs(panel: Panel, handle: Entity<Root>, border: gpui::Hsla) -> impl IntoElement {
    let mut row = div()
        .flex()
        .flex_row()
        .w_full()
        .border_b_1()
        .border_color(border);
    for (value, label) in [
        (Panel::Files, "Files"),
        (Panel::Write, "Write"),
        (Panel::Links, "Links"),
    ] {
        let select = handle.clone();
        row = row.child(
            div()
                .id(SharedString::from(format!("note-panel-{label}")))
                .flex_1()
                .py(px(7.0))
                .text_center()
                .text_size(px(12.0))
                .cursor_pointer()
                .tab_index(0)
                .role(gpui::accesskit::Role::Tab)
                .aria_label(SharedString::from(label))
                .aria_selected(value == panel)
                .when(value == panel, |element| element.border_b_2())
                .on_click(move |_, _, cx| {
                    select.update(cx, |root, cx| {
                        root.set_note_panel(value, cx);
                        cx.notify();
                    });
                })
                .child(label),
        );
    }
    row
}

fn empty(handle: Entity<Root>) -> impl IntoElement {
    let create = handle;
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .flex_1()
        .gap_3()
        .child(Title::new("No notes yet").order(2))
        .child(Text::new("Create a Markdown note in this project vault.").dimmed())
        .child(
            Button::new("create-first-note", "Create note")
                .variant(Variant::Filled)
                .on_click(move |_, _, cx| {
                    create.update(cx, |root, cx| {
                        root.create_note(notes::Template::Blank, cx);
                        cx.notify();
                    });
                }),
        )
}
