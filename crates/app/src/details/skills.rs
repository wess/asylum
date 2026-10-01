//! Skills: the shared library, switched on per Bot.

use super::{section, Details};
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString};
use guise::{Badge, Button, Size, Switch, Variant};

pub fn render(d: &mut Details, cx: &mut Context<Details>) -> AnyElement {
  let ink = ink(cx);
  let Some(bid) = d.bot_id() else { return div().into_any_element() };
  let mut col = div().flex().flex_col().gap(px(6.0)).child(section(t("Skills"), cx)).child(
    div().text_size(px(12.0)).text_color(ink.dimmed).pb(px(4.0)).child(t("One library shared by every Bot. Switch on the ones this Bot may use, or reference any with / in the composer.")),
  );
  if d.data.skills.is_empty() {
    col = col.child(div().text_size(px(13.0)).text_color(ink.dimmed).child(t("No skills yet. Ask a Bot to save a process as a skill, teach one by demonstration, or add one from the Marketplace.")));
  }
  for s in d.data.skills.clone() {
    let on = d.data.enabled.contains(&s.id);
    let (sid, b) = (s.id.clone(), bid.clone());
    col = col.child(
      div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .p(px(8.0))
        .rounded(px(8.0))
        .bg(ink.surface)
        .child(
          div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(div().flex().gap(px(6.0)).items_center().child(format!("/{}", s.name)).when(s.packaged, |d| d.child(Badge::new(t("Marketplace")).size(Size::Xs).variant(Variant::Light))).when(s.source == "taught", |d| d.child(Badge::new(t("Taught")).size(Size::Xs).variant(Variant::Light))))
            .child(div().truncate().text_size(px(12.0)).text_color(ink.dimmed).child(SharedString::from(s.description.clone()))),
        )
        .child(Switch::new(SharedString::from(format!("sk-{}", s.id))).checked(on).color(guise::ColorName::Violet).on_change(cx.listener(move |this, _, _, cx| {
          let rt = this.rt.clone();
          let (b, sid) = (b.clone(), sid.clone());
          this.run(cx, async move { store::skills::enable(&rt.pool, &b, &sid, !on).await }, |this, _, cx| this.load(cx));
        }))),
    );
  }
  col.child(Button::new("browse-skills", t("Browse Marketplace")).size(Size::Xs).variant(Variant::Light).on_click(|_, w, cx| w.dispatch_action(Box::new(crate::actions::Marketplace), cx))).into_any_element()
}
