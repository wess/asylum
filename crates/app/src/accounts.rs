//! The Accounts surface: provider accounts with usage bars and the active
//! account marked. Explicit actions switch or delete an account.

use gpui::prelude::*;
use gpui::{div, px, App, Entity, IntoElement, SharedString, Window};
use guise::prelude::*;

use crate::control::{empty, Button};
use crate::state::{AccountRow, Root};

pub fn accounts_view(
    rows: Vec<AccountRow>,
    input: Entity<guise::TextInput>,
    handle: Entity<Root>,
    _window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let _ = cx;
    let mut col = div()
        .id("accounts-scroll")
        .flex()
        .flex_col()
        .size_full()
        .gap_4()
        .p(px(20.0))
        .overflow_y_scroll();
    col = col.child(Title::new("Accounts").order(2));
    col = col.child(
        Text::new("Accounts are local labels for provider identities and their usage history. The active account marks your preferred identity in Asylum; it does not sign in or change the provider CLI session. Authentication stays with each provider’s CLI.")
            .size(Size::Sm)
            .dimmed(),
    );

    // Add-account form: provider and optional label, Enter or the button adds.
    let add = handle.clone();
    col = col.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(div().flex_1().min_w_0().child(input))
            .child(
                Button::new("add-account", "Add account")
                    .size(Size::Sm)
                    .variant(Variant::Filled)
                    .on_click(move |_, _, cx| {
                        add.update(cx, |root, cx| root.add_account_from_input(cx));
                    }),
            ),
    );

    if rows.is_empty() {
        return col.child(empty(
            "Connect your first account",
            "Enter a provider and optional label above to organize work and personal identities and keep their usage records separate.",
        ));
    }

    for row in rows {
        col = col.child(account_card(row, handle.clone()));
    }
    col
}

fn account_card(row: AccountRow, handle: Entity<Root>) -> impl IntoElement {
    let id = row.account.id;
    let active = row.account.active;
    let provider = row.account.provider.clone();

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
                Badge::new("available")
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

    let activate = handle.clone();
    let remove = handle;
    card = card.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .children((!active).then(|| {
                Button::new(
                    SharedString::from(format!("activate-account-{id}")),
                    "Use account",
                )
                .size(Size::Xs)
                .variant(Variant::Filled)
                .on_click(move |_, _, cx| {
                    activate.update(cx, |root, cx| {
                        if let Err(error) = root.db.activate_account(id) {
                            root.push_error("Could not switch account", error.to_string());
                        }
                        cx.notify();
                    });
                })
            }))
            .child(
                Button::new(SharedString::from(format!("delete-account-{id}")), "Delete")
                    .size(Size::Xs)
                    .variant(Variant::Subtle)
                    .on_click(move |_, _, cx| {
                        remove.update(cx, |root, cx| {
                            root.confirm = Some(crate::run::ConfirmAction::DeleteAccount(id));
                            cx.notify();
                        });
                    }),
            ),
    );
    Card::new().padding(Size::Md).child(card)
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
