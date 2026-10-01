//! The secure secret card: a masked field whose value goes to the keychain
//! and is never shown to the Bot.

use super::{key, shell};
use crate::chat::ChatPane;
use crate::i18n::t;
use crate::theme::ink;
use agent::Part;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString};
use guise::{Badge, Button, Group, IconName, Size, Variant};
use store::Message;

pub fn render(pane: &mut ChatPane, m: &Message, i: usize, p: &Part, cx: &mut Context<ChatPane>) -> AnyElement {
  let Part::SecretRequest { name, description, status, fill } = p else { return div().into_any_element() };
  let ink = ink(cx);
  let mut card = shell(cx)
    .child(
      div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(div().text_color(ink.primary).child(guise::Icon::new(IconName::KeyRound).size(Size::Sm)))
        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(SharedString::from(name.clone())))
        .child(div().flex_1())
        .child(match status.as_str() {
          "saved" => Badge::new(t("Saved")).color(guise::ColorName::Green).size(Size::Xs),
          "filled" => Badge::new(t("Filled")).color(guise::ColorName::Green).size(Size::Xs),
          "failed" => Badge::new(t("Not filled")).color(guise::ColorName::Red).size(Size::Xs),
          "skipped" => Badge::new(t("Skipped")).color(guise::ColorName::Gray).size(Size::Xs),
          _ => Badge::new(t("Needed")).color(guise::ColorName::Violet).size(Size::Xs),
        }),
    )
    .child(div().text_size(px(13.0)).text_color(ink.dimmed).child(SharedString::from(description.clone())));
  match status.as_str() {
    "pending" => {
      let k = key(m, i, "value");
      let input = pane.cards.line(k.clone(), "", true, cx);
      let bot = m.bot_id.clone().unwrap_or_default();
      let (n1, d1, f1) = (name.clone(), description.clone(), *fill);
      let (bot2, n2) = (bot.clone(), name.clone());
      card = card.child(input).child(
        Group::new()
          .gap(Size::Sm)
          .child(Button::new(SharedString::from(format!("save-{k}")), t("Save securely")).size(Size::Xs).on_click(cx.listener(move |this, _, _, cx| {
            let value = this.cards.text(&k, cx);
            let rt = this.rt.clone();
            let (bot, n, d) = (bot.clone(), n1.clone(), d1.clone());
            this.run(cx, async move { agent::api::cards::save_secret(&rt, &bot, &n, &d, &value, f1).await }, |p, _, cx| p.load(cx));
          })))
          .child(Button::new(SharedString::from(format!("skip-{}", m.id)), t("Skip")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(move |this, _, _, cx| {
            let rt = this.rt.clone();
            let (bot, n) = (bot2.clone(), n2.clone());
            this.run(cx, async move { agent::api::cards::skip_secret(&rt, &bot, &n).await }, |_, _, _| {});
          }))),
      );
    }
    "saved" => card = card.child(div().text_size(px(12.0)).text_color(ink.success).child(t("Saved securely and kept private"))),
    "filled" => card = card.child(div().text_size(px(12.0)).text_color(ink.success).child(t("Filled into the page. Secret values were never shown to your Bot."))),
    "failed" => card = card.child(div().text_size(px(12.0)).text_color(ink.danger).child(t("Could not fill into the page"))),
    _ => {}
  }
  card.child(div().text_size(px(11.0)).text_color(ink.dimmed).child(t("Stored securely, never shown to your Bot"))).into_any_element()
}
