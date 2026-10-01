//! "Action needed" on the computer: take over to handle a password, 2FA,
//! CAPTCHA, or payment yourself, or skip.

use super::shell;
use crate::chat::ChatPane;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString};
use guise::{Badge, Button, Group, IconName, Size, Variant};
use store::Message;

pub fn render(pane: &mut ChatPane, m: &Message, reason: &str, status: &str, cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  let bot = m.bot_id.clone().unwrap_or_default();
  let mut card = shell(cx)
    .border_color(if status == "pending" { ink.warning } else { ink.border })
    .child(
      div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(div().text_color(ink.warning).child(guise::Icon::new(IconName::Monitor).size(Size::Sm)))
        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(t("Computer")))
        .child(Badge::new(match status {
          "done" => t("Done"),
          "skipped" => t("Skipped"),
          _ => t("Action needed"),
        }).size(Size::Xs).color(if status == "pending" { guise::ColorName::Yellow } else { guise::ColorName::Gray })),
    )
    .child(div().text_size(px(13.0)).child(SharedString::from(reason.to_string())));
  if status == "pending" {
    let b2 = bot.clone();
    card = card.child(
      Group::new()
        .gap(Size::Sm)
        .child(Button::new(SharedString::from(format!("take-{}", m.id)), t("Take over")).size(Size::Xs).on_click(cx.listener(move |this, _, w, cx| {
          let bot = bot.clone();
          this.with_root(cx, |r, cx| {
            if r.right != crate::root::Right::Computer {
              r.show_computer(w, cx);
            }
            if let Some(p) = r.panel.clone() {
              p.update(cx, |p, cx| p.takeover(true, &bot, cx));
            }
          });
        })))
        .child(Button::new(SharedString::from(format!("skip-{}", m.id)), t("Skip")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(move |this, _, _, cx| {
          let rt = this.rt.clone();
          let bot = b2.clone();
          this.run(cx, async move { agent::api::cards::takeover(&rt, &bot, false).await }, |_, _, _| {});
        }))),
    );
  }
  let _ = pane;
  card.into_any_element()
}
