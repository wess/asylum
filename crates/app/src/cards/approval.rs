//! Approval cards: the proposed operation, its target, and its inputs, with
//! Allow once / Always allow / Deny. Auto-review raises "Review an action".

use super::shell;
use crate::chat::ChatPane;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString};
use guise::{Badge, Button, Code, Group, Size, Variant};

pub fn render(pane: &mut ChatPane, id: &str, cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  let Some(a) = pane.approvals.get(id).cloned() else {
    return shell(cx).child(t("Approval")).into_any_element();
  };
  let title = match a.kind.as_str() {
    "review" => t("Review an action"),
    "local" => t("Run on your local computer?"),
    "connector" => t("Allow a connector"),
    _ => t("Approval needed"),
  };
  let op = crate::cards::tool::label(&a.tool, &serde_json::from_str(&a.args).unwrap_or_default());
  let mut card = shell(cx)
    .border_color(if a.status == "pending" { ink.warning } else { ink.border })
    .child(
      div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(div().text_color(ink.warning).child(guise::Icon::new(guise::IconName::ShieldAlert).size(Size::Sm)))
        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
        .child(div().flex_1())
        .child(match a.status.as_str() {
          "approved" => Badge::new(t("Allowed")).color(guise::ColorName::Green).size(Size::Xs),
          "denied" => Badge::new(t("Denied")).color(guise::ColorName::Red).size(Size::Xs),
          "expired" => Badge::new(t("Expired")).color(guise::ColorName::Gray).size(Size::Xs),
          "canceled" => Badge::new(t("Canceled")).color(guise::ColorName::Gray).size(Size::Xs),
          _ => Badge::new(t("Pending")).color(guise::ColorName::Yellow).size(Size::Xs),
        }),
    )
    .child(div().text_size(px(13.0)).child(SharedString::from(op)));
  if !a.target.is_empty() {
    card = card.child(Code::new(a.target.clone()));
  }
  if !a.reason.is_empty() {
    card = card.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(SharedString::from(a.reason.clone())));
  }
  if a.args.len() > 2 {
    card = card.child(
      div()
        .max_h(px(160.0))
        .overflow_hidden()
        .text_size(px(12.0))
        .font_family("Menlo")
        .text_color(ink.dimmed)
        .child(SharedString::from(a.args.clone())),
    );
  }
  if a.status == "pending" && a.kind == "local" {
    let (i1, i2, i3, i4) = (a.id.clone(), a.id.clone(), a.id.clone(), a.id.clone());
    // The admin's ceiling keeps "Always allow" off.
    let capped = capped_by_policy(&pane.rt);
    card = card
      .child(div().text_size(px(12.5)).child(t("Allow Asylum and all Bots to run commands on your local computer?")))
      .child(
        Group::new()
          .gap(Size::Sm)
          .wrap(true)
          .child(Button::new(SharedString::from(format!("lalways-{}", a.id)), t("Always allow")).size(Size::Xs).disabled(capped).on_click(cx.listener(move |this, _, _, cx| {
            crate::settings::save(&this.rt, |s| s.local_exec = config::LocalExec::Always);
            decide(this, &i1, true, false, cx)
          })))
          .child(Button::new(SharedString::from(format!("lonce-{}", a.id)), t("Allow once")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| decide(this, &i2, true, false, cx))))
          .child(Button::new(SharedString::from(format!("lnever-{}", a.id)), t("Never")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(move |this, _, _, cx| {
            crate::settings::save(&this.rt, |s| s.local_exec = config::LocalExec::Never);
            decide(this, &i3, false, false, cx)
          })))
          .child(Button::new(SharedString::from(format!("ldeny-{}", a.id)), t("Deny once")).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(move |this, _, _, cx| decide(this, &i4, false, false, cx)))),
      );
  } else if a.status == "pending" {
    let (i1, i2, i3) = (a.id.clone(), a.id.clone(), a.id.clone());
    card = card.child(
      Group::new()
        .gap(Size::Sm)
        .child(Button::new(SharedString::from(format!("once-{}", a.id)), t("Allow once")).size(Size::Xs).on_click(cx.listener(move |this, _, _, cx| decide(this, &i1, true, false, cx))))
        .child(
          Button::new(SharedString::from(format!("always-{}", a.id)), t("Always allow"))
            .size(Size::Xs)
            .variant(Variant::Light)
            .on_click(cx.listener(move |this, _, _, cx| decide(this, &i2, true, true, cx))),
        )
        .child(
          Button::new(SharedString::from(format!("deny-{}", a.id)), t("Deny"))
            .size(Size::Xs)
            .variant(Variant::Default)
            .on_click(cx.listener(move |this, _, _, cx| decide(this, &i3, false, false, cx))),
        ),
    );
  } else if a.status == "expired" {
    card = card.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("This expired because nobody answered in time. Choose Always allow this in the future in Auto-review rules to skip it next time.")));
  }
  card.into_any_element()
}

fn decide(pane: &mut ChatPane, id: &str, allow: bool, always: bool, cx: &mut Context<ChatPane>) {
  let rt = pane.rt.clone();
  let id = id.to_string();
  pane.run(cx, async move { agent::api::cards::approve(&rt, &id, allow, always).await }, |p, _, cx| p.load(cx));
}

fn capped_by_policy(rt: &agent::Runtime) -> bool {
  rt.policy().cap_local(config::LocalExec::Always) != config::LocalExec::Always
}
