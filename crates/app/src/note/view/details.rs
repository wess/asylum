use gpui::prelude::*;
use gpui::{div, px, Entity, IntoElement, SharedString};
use guise::prelude::*;

use crate::icons::icon_button;
use crate::state::Root;

use super::Palette;

pub(super) fn pane(
    root: &Root,
    handle: Entity<Root>,
    compact: bool,
    palette: Palette,
) -> impl IntoElement {
    let note = root.current_note().cloned();
    let backlinks: Vec<notes::Note> = note
        .as_ref()
        .map(|note| {
            root.note
                .index
                .backlinks(note)
                .into_iter()
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let outgoing: Vec<notes::Note> = note
        .as_ref()
        .map(|note| {
            root.note
                .index
                .outgoing(note)
                .into_iter()
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let attachments = root
        .project_id
        .zip(note.as_ref())
        .and_then(|(project_id, note)| root.db.note_attachments(project_id, &note.path).ok())
        .unwrap_or_default();
    let mut pane = div()
        .flex()
        .flex_col()
        .h_full()
        .min_h_0()
        .text_color(palette.text)
        .when(!compact, |element| {
            element
                .w(px(224.0))
                .border_l_1()
                .border_color(palette.border)
                .flex_none()
        });
    if !compact {
        let collapse = handle.clone();
        pane = pane.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .h(px(36.0))
                .px(px(10.0))
                .border_b_1()
                .border_color(palette.border)
                .child(Text::new("Properties").size(Size::Xs).bold())
                .child(div().ml_auto().child(icon_button(
                    "collapse-note-properties",
                    "chevron-right",
                    "Hide properties",
                    palette.dimmed,
                    palette.hover,
                    move |_, cx| {
                        collapse.update(cx, |root, cx| {
                            root.note.details_open = false;
                            cx.notify();
                        });
                    },
                ))),
        );
    }

    let mut content = div()
        .id("note-details-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .p(px(10.0))
        .gap_1();
    let Some(note) = note else {
        return pane.child(content.child(
            Text::new("Select a note to see its properties, links, backlinks, tags, and attached task context.")
                .size(Size::Xs)
                .dimmed(),
        )).into_any_element();
    };

    let mut properties = detailsection(if compact { "Properties" } else { "Frontmatter" });
    if note.properties.is_empty() {
        properties = properties.child(
            Text::new("Properties add structured details such as status, type, or owner. Use name: value below to add one.")
                .size(Size::Xs)
                .dimmed(),
        );
    }
    for property in note.properties {
        let name = property.name.clone();
        let remove = handle.clone();
        properties = properties.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(70.0))
                        .flex_none()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(11.0))
                        .text_color(palette.dimmed)
                        .child(SharedString::from(property.name)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_size(px(11.0))
                        .text_color(palette.text)
                        .child(SharedString::from(property.value)),
                )
                .child(icon_button(
                    format!("remove-prop-{name}"),
                    "x",
                    "Remove property",
                    palette.dimmed,
                    palette.hover,
                    move |_, cx| {
                        remove.update(cx, |root, cx| root.remove_note_property(&name, cx));
                    },
                )),
        );
    }
    // Add a property via a `name: value` input (Enter to apply).
    if let Some(input) = root.note.property_input.clone() {
        properties = properties.child(div().pt(px(2.0)).child(input));
    }
    content = content.child(properties);

    let mut tags = detailsection("Tags").flex_row().flex_wrap();
    if note.tags.is_empty() {
        tags = tags.child(
            Text::new("Tags group related notes and make them easier to filter.")
                .size(Size::Xs)
                .dimmed(),
        );
    }
    for tag in note.tags {
        let value = tag.clone();
        let filter = handle.clone();
        tags = tags.child(
            div()
                .id(SharedString::from(format!("tag-{tag}")))
                .cursor_pointer()
                .tab_index(0)
                .role(gpui::accesskit::Role::Button)
                .aria_label(SharedString::from(format!("Filter notes by tag {tag}")))
                .on_click(move |_, _, cx| {
                    let value = value.clone();
                    filter.update(cx, |root, cx| root.set_note_tag_filter(value, cx));
                })
                .child(Badge::new(SharedString::from(format!("#{tag}")))),
        );
    }
    content = content.child(tags);
    content = content.child(linksection("Links", outgoing, handle.clone()));
    content = content.child(linksection("Backlinks", backlinks, handle));

    let mut attached = detailsection("Attached context");
    if attachments.is_empty() {
        attached = attached.child(
            Text::new("Attach this note to a task or run to include its context in agent prompts.")
                .size(Size::Xs)
                .dimmed(),
        );
    }
    for attachment in attachments {
        let label = match (attachment.task_id, attachment.run_id) {
            (Some(id), _) => format!("Task #{id}"),
            (_, Some(id)) => format!("Run #{id}"),
            _ => continue,
        };
        attached = attached.child(Text::new(SharedString::from(label)).size(Size::Xs));
    }
    content = content.child(attached);
    pane.child(content).into_any_element()
}

fn detailsection(title: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .py(px(9.0))
        .border_b_1()
        .child(Text::new(title).size(Size::Xs).bold())
}

fn linksection(title: &'static str, notes: Vec<notes::Note>, handle: Entity<Root>) -> gpui::Div {
    let mut section = detailsection(title);
    if notes.is_empty() {
        let detail = if title == "Backlinks" {
            "Notes that link to this note will appear here."
        } else {
            "Add a [[wiki link]] in the note to connect related ideas."
        };
        return section.child(Text::new(detail).size(Size::Xs).dimmed());
    }
    for note in notes {
        let path = note.path.clone();
        let select = handle.clone();
        section = section.child(
            div()
                .id(SharedString::from(format!("{title}-{}", note.path)))
                .cursor_pointer()
                .tab_index(0)
                .role(gpui::accesskit::Role::Link)
                .aria_label(SharedString::from(format!("Open note {}", note.title)))
                .on_click(move |_, _, cx| {
                    select.update(cx, |root, cx| root.select_note(&path, cx));
                })
                .child(Text::new(SharedString::from(note.title)).size(Size::Xs)),
        );
    }
    section
}
