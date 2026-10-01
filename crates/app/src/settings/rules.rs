//! Auto-review rules: "Ask first" and "Allow automatically", in plain
//! language. Ask first wins on conflict; locked rows come from a team.

use super::{heading, Dialog};
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString};
use guise::{ActionIcon, Badge, Button, Group, IconName, Size, Variant};

pub fn add(d: &mut Dialog, kind: &'static str, text: String, cx: &mut Context<Dialog>) {
  if text.trim().is_empty() {
    return;
  }
  let rt = d.rt.clone();
  cx.spawn(async move |this, cx| {
    let _ = crate::tk::run(async move {
      let r = store::rules::add(&rt.pool, kind, &text, false).await;
      agent::audit::change(&rt, "user", "rule.added", kind, &text).await;
      r
    })
    .await;
    let _ = this.update(cx, |this, cx| {
      this.rule.update(cx, |i, cx| i.set_text("", cx));
      this.refresh(cx);
    });
  })
  .detach();
}

pub fn render(d: &mut Dialog, cx: &mut Context<Dialog>) -> AnyElement {
  let ink = ink(cx);
  let mut col = div().flex().flex_col().gap(px(6.0)).child(heading(t("Auto-review Rules"), cx)).child(
    div().text_size(px(12.5)).text_color(ink.dimmed).pb(px(6.0)).child(t("Ask first rules win over Allow automatically. Allow rules apply only when nothing else is a reason to stop.")),
  );
  for r in d.rules.clone() {
    let id = r.id.clone();
    col = col.child(
      div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .p(px(8.0))
        .rounded(px(8.0))
        .bg(ink.surface)
        .child(Badge::new(if r.kind == "ask" { t("Ask first") } else { t("Allow automatically") }).size(Size::Xs).color(if r.kind == "ask" { guise::ColorName::Yellow } else { guise::ColorName::Green }))
        .child(div().flex_1().text_size(px(13.0)).child(SharedString::from(r.text.clone())))
        .when(r.locked, |d| d.child(div().id(SharedString::from(format!("lock-{}", r.id))).tooltip(guise::tooltip(t("Required by your admin. You can't edit or delete this rule."))).text_color(ink.dimmed).child(guise::Icon::new(IconName::Lock).size(Size::Xs))))
        .when(!r.locked, |d| {
          d.child(ActionIcon::new(SharedString::from(format!("del-{}", r.id)), IconName::Trash2).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(move |this, _, _, cx| {
            let rt = this.rt.clone();
            let id = id.clone();
            cx.spawn(async move |this, cx| {
              let _ = crate::tk::run(async move {
                store::rules::delete(&rt.pool, &id).await?;
                agent::audit::change(&rt, "user", "rule.deleted", &id, "").await;
                anyhow::Ok(())
              })
              .await;
              let _ = this.update(cx, |this, cx| this.refresh(cx));
            })
            .detach();
          })))
        }),
    );
  }
  if d.rules.is_empty() {
    col = col.child(div().text_size(px(13.0)).text_color(ink.dimmed).child(t("No rules yet.")));
  }
  col
    .child(div().pt(px(10.0)).child(d.rule.clone()))
    .child(
      Group::new()
        .gap(Size::Sm)
        .child(Button::new("add-ask", t("Add Ask first")).size(Size::Xs).on_click(cx.listener(|this, _, _, cx| {
          let text = this.rule.read(cx).text();
          add(this, "ask", text, cx);
        })))
        .child(Button::new("add-allow", t("Add Allow automatically")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, _, cx| {
          let text = this.rule.read(cx).text();
          add(this, "allow", text, cx);
        }))),
    )
    .into_any_element()
}
