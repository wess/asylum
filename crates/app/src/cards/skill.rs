//! A skill drafted from a Teach-a-task demo: review, edit, save — or test
//! it on a safe example first.

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
  let Part::SkillDraft { name, description, instructions, status } = p else { return div().into_any_element() };
  let ink = ink(cx);
  let card = shell(cx).child(
    div()
      .flex()
      .items_center()
      .gap(px(8.0))
      .child(div().text_color(ink.primary).child(guise::Icon::new(IconName::GraduationCap).size(Size::Sm)))
      .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(t("Skill draft")))
      .child(div().flex_1())
      .when(status != "draft", |d| d.child(Badge::new(if status == "saved" { t("Saved") } else { t("Discarded") }).size(Size::Xs))),
  );
  if status != "draft" {
    return card
      .child(div().font_weight(gpui::FontWeight::MEDIUM).child(SharedString::from(name.clone())))
      .child(div().text_size(px(13.0)).text_color(ink.dimmed).child(SharedString::from(description.clone())))
      .into_any_element();
  }
  let (kn, kd, ki) = (key(m, i, "name"), key(m, i, "description"), key(m, i, "instructions"));
  let n = pane.cards.line(kn.clone(), name, false, cx);
  let d = pane.cards.line(kd.clone(), description, false, cx);
  let ins = pane.cards.area(ki.clone(), instructions, 8, cx);
  let (mid, mid2) = (m.id.clone(), m.id.clone());
  let (kn2, kd2, ki2) = (kn.clone(), kd.clone(), ki.clone());
  card
    .child(n)
    .child(d)
    .child(ins)
    .child(div().text_size(px(11.5)).text_color(ink.dimmed).child(t("Add decision rules, failure handling, and what needs your approval before saving.")))
    .child(
      Group::new()
        .gap(Size::Sm)
        .child(Button::new(SharedString::from(format!("saveskill-{}", m.id)), t("Save skill")).size(Size::Xs).on_click(cx.listener(move |this, _, _, cx| {
          let (n, d, i2) = (this.cards.text(&kn, cx), this.cards.text(&kd, cx), this.cards.text(&ki, cx));
          let rt = this.rt.clone();
          let mid = mid.clone();
          this.run(cx, async move { agent::api::cards::save_skill_draft(&rt, &mid, i, &n, &d, &i2, true).await }, |p, _, cx| p.load(cx));
        })))
        .child(Button::new(SharedString::from(format!("testskill-{}", m.id)), t("Test on a safe example")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| {
          let (n, d, i2) = (this.cards.text(&kn2, cx), this.cards.text(&kd2, cx), this.cards.text(&ki2, cx));
          let text = format!("Test this skill on a safe example (don't send, post, or buy anything), then tell me how it went.\n\n# {n}\n{d}\n\n{i2}");
          let rt = this.rt.clone();
          let c = this.id.clone();
          this.run(cx, async move { agent::api::chat::send(&rt, &c, &text, &[], None).await }, |_, _, _| {});
        })))
        .child(Button::new(SharedString::from(format!("discardskill-{}", m.id)), t("Discard")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(move |this, _, _, cx| {
          let rt = this.rt.clone();
          let mid = mid2.clone();
          this.run(cx, async move { agent::api::cards::save_skill_draft(&rt, &mid, i, "", "", "", false).await }, |p, _, cx| p.load(cx));
        }))),
    )
    .into_any_element()
}
