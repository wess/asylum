//! The Inbox surface: notifications newest-first, unread ones marked. A button
//! marks everything read.

use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::control::Button;
use crate::state::Root;
use store::Notification;

pub fn inbox_view(
    items: Vec<Notification>,
    handle: Entity<Root>,
    _window: &mut Window,
    _cx: &mut App,
) -> impl IntoElement {
    let mut col = div().flex().flex_col().w_full().gap_4().p(px(20.0));

    let clear = handle.clone();
    col = col.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(Title::new("Inbox").order(2))
            .child(
                Button::new("mark-all", "Mark all read")
                    .size(Size::Xs)
                    .variant(Variant::Subtle)
                    .on_click(move |_, _, cx| {
                        clear.update(cx, |root, cx| {
                            if let Err(error) = root.db.mark_all_read() {
                                root.push_error("Could not update inbox", error.to_string());
                            }
                            cx.notify();
                        });
                    }),
            ),
    );

    if items.is_empty() {
        return col.child(Text::new("You're all caught up.").size(Size::Sm).dimmed());
    }

    for n in items {
        col = col.child(notification_row(n, handle.clone()));
    }
    col
}

fn notification_row(n: Notification, handle: Entity<Root>) -> impl IntoElement {
    let id = n.id;
    let label = format!("Mark {} as read", n.title);
    let color = match n.kind.as_str() {
        "check_failed" | "run_failed" => ColorName::Red,
        "attention" => ColorName::Orange,
        _ => ColorName::Blue,
    };
    let dot = if n.read { "○" } else { "●" };

    div()
        .id(SharedString::from(format!("notif-{id}")))
        .cursor_pointer()
        .tab_index(0)
        .role(gpui::accesskit::Role::Button)
        .aria_label(SharedString::from(label))
        .focus_visible(|style| style.border_1().border_color(gpui::rgb(0x3b82f6)))
        .on_click(move |_, _, cx| {
            handle.update(cx, |root, cx| {
                if let Err(error) = root.db.mark_read(id, true) {
                    root.push_error("Could not update notification", error.to_string());
                }
                cx.notify();
            });
        })
        .child(
            Card::new().padding(Size::Sm).child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(Text::new(SharedString::from(dot)))
                    .child(
                        Badge::new(SharedString::from(n.kind.clone()))
                            .color(color)
                            .variant(Variant::Light),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(Text::new(SharedString::from(n.title.clone())).bold())
                            .child(
                                Text::new(SharedString::from(n.body.clone()))
                                    .size(Size::Xs)
                                    .dimmed(),
                            ),
                    ),
            ),
        )
}
