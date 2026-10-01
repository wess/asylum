//! Drafts to approve: New Email and New Slack Message, editable before
//! sending.

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
  let Part::Draft { kind, to, subject, body, status, channel } = p else { return div().into_any_element() };
  let ink = ink(cx);
  let email = kind == "email";
  let title = if email { t("New Email") } else { t("New Slack Message") };
  let mut card = shell(cx).child(
    div()
      .flex()
      .items_center()
      .gap(px(8.0))
      .child(div().text_color(ink.primary).child(guise::Icon::new(if email { IconName::Mail } else { IconName::MessageSquare }).size(Size::Sm)))
      .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
      .child(div().flex_1())
      .when(status != "draft", |d| {
        d.child(Badge::new(match status.as_str() {
          "sent" => t("Sent"),
          "opened" => t("Opened in Mail"),
          _ => t("Discarded"),
        }).size(Size::Xs))
      }),
  );
  let label = |s: &'static str| div().w(px(64.0)).flex_none().text_size(px(12.0)).text_color(ink.dimmed).child(t(s));
  if status != "draft" {
    if email {
      card = card
        .child(div().flex().child(label("To")).child(div().text_size(px(13.0)).child(to.join(", "))))
        .child(div().flex().child(label("Subject")).child(div().text_size(px(13.0)).child(SharedString::from(subject.clone()))));
    } else {
      card = card.child(div().flex().child(label("Channel")).child(div().text_size(px(13.0)).child(SharedString::from(channel.clone()))));
    }
    return card.child(div().text_size(px(13.0)).child(SharedString::from(body.clone()))).into_any_element();
  }
  let (kt, ks, kb, kc) = (key(m, i, "to"), key(m, i, "subject"), key(m, i, "body"), key(m, i, "channel"));
  if email {
    let to_in = pane.cards.line(kt.clone(), &to.join(", "), false, cx);
    let subj = pane.cards.line(ks.clone(), subject, false, cx);
    card = card.child(div().flex().items_center().child(label("To")).child(div().flex_1().child(to_in))).child(div().flex().items_center().child(label("Subject")).child(div().flex_1().child(subj)));
  } else {
    let ch = pane.cards.line(kc.clone(), channel, false, cx);
    card = card.child(div().flex().items_center().child(label("Channel")).child(div().flex_1().child(ch)));
  }
  let area = pane.cards.area(kb.clone(), body, 6, cx);
  let mid = m.id.clone();
  let mid2 = m.id.clone();
  card
    .child(area)
    .child(
      Group::new()
        .gap(Size::Sm)
        .child(Button::new(SharedString::from(format!("send-{}", m.id)), if email { t("Send email") } else { t("Send message") }).size(Size::Xs).on_click(cx.listener(move |this, _, _, cx| {
          let to: Vec<String> = this.cards.text(&kt, cx).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
          let (subject, body, channel) = (this.cards.text(&ks, cx), this.cards.text(&kb, cx), this.cards.text(&kc, cx));
          let rt = this.rt.clone();
          let mid = mid.clone();
          this.run(cx, async move { agent::api::cards::send_draft(&rt, &mid, i, to, &subject, &body, &channel).await }, |p, _, cx| p.load(cx));
        })))
        .child(Button::new(SharedString::from(format!("discard-{}", m.id)), t("Discard")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(move |this, _, _, cx| {
          let rt = this.rt.clone();
          let mid = mid2.clone();
          this.run(cx, async move { agent::api::cards::discard_draft(&rt, &mid, i).await }, |p, _, cx| p.load(cx));
        }))),
    )
    .into_any_element()
}
