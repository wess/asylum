//! What a Bot remembers. Correct it by telling the Bot, or remove entries.

use super::{section, Details};
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString};
use guise::{ActionIcon, Badge, Button, IconName, Size, Variant};

pub fn render(d: &mut Details, cx: &mut Context<Details>) -> AnyElement {
  let ink = ink(cx);
  let mut col = div().flex().flex_col().gap(px(6.0)).child(section(t("Memory"), cx));
  if d.data.memory.is_empty() {
    col = col.child(div().text_size(px(13.0)).text_color(ink.dimmed).child(t("Nothing remembered yet. Agents keep stable preferences, important facts, and summaries of finished work.")));
  }
  for m in d.data.memory.clone() {
    let id = m.id.clone();
    let scope = match m.scope.as_str() {
      "team" => Some(t("team")),
      "person" => Some(t("about you")),
      _ => None,
    };
    col = col.child(
      div()
        .flex()
        .items_start()
        .gap(px(8.0))
        .p(px(8.0))
        .rounded(px(8.0))
        .bg(ink.surface)
        .child(Badge::new(t(match m.kind.as_str() {
          "preference" => "preference",
          "summary" => "summary",
          _ => "fact",
        })).size(Size::Xs).variant(Variant::Light))
        .when_some(scope, |d, s| d.child(Badge::new(s).size(Size::Xs)))
        .child(div().flex_1().text_size(px(13.0)).child(SharedString::from(m.content.clone())))
        .child(ActionIcon::new(SharedString::from(format!("forget-{}", m.id)), IconName::Trash2).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(move |this, _, _, cx| {
          let rt = this.rt.clone();
          let id = id.clone();
          this.run(cx, async move { store::memories::delete(&rt.pool, &id).await }, |this, _, cx| this.load(cx));
        }))),
    );
  }
  if !d.data.memory.is_empty() {
    let bid = d.bot_id().unwrap_or_default();
    col = col.child(Button::new("forget-all", t("Forget everything")).size(Size::Xs).variant(Variant::Subtle).color(guise::ColorName::Red).on_click(cx.listener(move |this, _, _, cx| {
      let rt = this.rt.clone();
      let b = bid.clone();
      this.run(cx, async move { store::memories::clear(&rt.pool, &b).await }, |this, _, cx| this.load(cx));
    })));
  }
  col.into_any_element()
}
