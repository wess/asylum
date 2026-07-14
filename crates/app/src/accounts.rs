//! The Accounts surface: provider accounts with usage bars and the active
//! account marked. Clicking an inactive account hot-swaps it.

use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::state::{AccountRow, Root};

pub fn accounts_view(
    rows: Vec<AccountRow>,
    handle: Entity<Root>,
    _window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let focus = guise::theme::theme(cx).primary().hsla();
    let mut col = div().flex().flex_col().w_full().gap_4().p(px(20.0));
    col = col.child(Title::new("Accounts").order(2));

    if rows.is_empty() {
        return col.child(
            Text::new("No accounts yet. Sign in to a provider to track usage.")
                .size(Size::Sm)
                .dimmed(),
        );
    }

    for row in rows {
        col = col.child(account_card(row, handle.clone(), focus));
    }
    col
}

fn account_card(row: AccountRow, handle: Entity<Root>, focus: gpui::Hsla) -> impl IntoElement {
    let id = row.account.id;
    let active = row.account.active;
    let provider = row.account.provider.clone();
    let account_label = row.account.label.clone();

    let mut card = div().flex().flex_col().gap_2().child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(Text::new(SharedString::from(provider.clone())).bold())
                    .child(
                        Text::new(SharedString::from(row.account.label.clone()))
                            .size(Size::Sm)
                            .dimmed(),
                    ),
            )
            .child(if active {
                Badge::new("active")
                    .color(ColorName::Green)
                    .variant(Variant::Light)
            } else {
                Badge::new("switch")
                    .color(ColorName::Gray)
                    .variant(Variant::Light)
            }),
    );

    if let Some(usage) = &row.usage {
        let pct = usage.fraction().map(|f| (f * 100.0) as u32).unwrap_or(0);
        let limit = usage
            .limit
            .map(|l| l.to_string())
            .unwrap_or_else(|| "∞".into());
        card = card.child(
            Text::new(SharedString::from(format!(
                "Usage: {} / {} ({pct}%)",
                usage.used, limit
            )))
            .size(Size::Xs)
            .dimmed(),
        );
        card = card.child(usage_bar(pct));
    }

    let clickable = div()
        .id(SharedString::from(format!("account-{id}")))
        .cursor_pointer()
        .tab_index(0)
        .role(gpui::accesskit::Role::Button)
        .aria_label(SharedString::from(format!(
            "Use {provider} account {account_label}"
        )))
        .focus_visible(move |style| style.border_1().border_color(focus))
        .on_click(move |_, _, cx| {
            handle.update(cx, |root, cx| {
                if let Err(error) = root.db.activate_account(id) {
                    root.push_error("Could not switch account", error.to_string());
                }
                cx.notify();
            });
        })
        .child(Card::new().padding(Size::Md).child(card));
    clickable
}

/// A slim usage meter, 0..100.
fn usage_bar(pct: u32) -> impl IntoElement {
    let filled = (pct.min(100)) as f32 / 100.0;
    div()
        .w_full()
        .h(px(6.0))
        .rounded(px(3.0))
        .bg(gpui::rgba(0x88888833))
        .child(
            div()
                .h_full()
                .rounded(px(3.0))
                .bg(gpui::rgba(0x3b82f6ff))
                .w(gpui::relative(filled)),
        )
}
