//! In-app error notices above the composer: dismiss one, clear all, or copy
//! the request ID.

use super::ChatPane;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, Context, SharedString};
use guise::{ActionIcon, IconName, Size, Variant};

pub fn render(pane: &mut ChatPane, cx: &mut Context<ChatPane>) -> impl IntoElement {
  let ink = ink(cx);
  if pane.notices.is_empty() {
    return div();
  }
  let mut col = div()
    .max_w(px(880.0))
    .w_full()
    .mx_auto()
    .px(px(24.0))
    .pb(px(8.0))
    .flex()
    .flex_col()
    .gap(px(6.0))
    .child(
      div()
        .flex()
        .justify_between()
        .text_size(px(11.0))
        .text_color(ink.dimmed)
        .child(t("Notifications"))
        .child(div().id("clear-notices").cursor_pointer().child(t("Clear")).on_click(cx.listener(|this, _, _, cx| {
          this.notices.clear();
          cx.notify();
        }))),
    );
  for (i, (text, request)) in pane.notices.clone().into_iter().enumerate() {
    let req = request.clone();
    col = col.child(
      div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .p(px(10.0))
        .rounded(px(8.0))
        .bg(ink.danger.opacity(0.1))
        .border_1()
        .border_color(ink.danger.opacity(0.3))
        .child(div().text_color(ink.danger).child(guise::Icon::new(IconName::CircleAlert).size(Size::Sm)))
        .child(div().flex_1().text_size(px(13.0)).child(SharedString::from(text)))
        .child(
          ActionIcon::new(SharedString::from(format!("copyreq{i}")), IconName::Copy)
            .variant(Variant::Subtle)
            .size(Size::Xs)
            .on_click(move |_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(req.clone()))),
        )
        .child(
          ActionIcon::new(SharedString::from(format!("dismiss{i}")), IconName::X)
            .variant(Variant::Subtle)
            .size(Size::Xs)
            .on_click(cx.listener(move |this, _, _, cx| {
              if i < this.notices.len() {
                this.notices.remove(i);
              }
              cx.notify();
            })),
        ),
    );
  }
  col
}
