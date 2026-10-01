//! Small cards: questions, reasoning, handoffs, routine events, connect
//! prompts, and errors.

use super::shell;
use crate::chat::ChatPane;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString};
use guise::{Button, Group, IconName, Size, Variant};
use store::Message;

pub fn question(pane: &mut ChatPane, m: &Message, text: &str, options: &[String], cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  let answered = pane.messages.iter().any(|x| x.created > m.created && x.role == "user");
  let mut card = shell(cx)
    .border_color(if answered { ink.border } else { ink.primary })
    .child(
      div()
        .flex()
        .gap(px(8.0))
        .items_center()
        .child(div().text_color(ink.primary).child(guise::Icon::new(IconName::MessageCircleQuestion).size(Size::Sm)))
        .child(div().font_weight(gpui::FontWeight::MEDIUM).child(SharedString::from(text.to_string()))),
    );
  if !options.is_empty() && !answered {
    let mut g = Group::new().gap(Size::Xs).wrap(true);
    for (i, o) in options.iter().enumerate() {
      let o2 = o.clone();
      g = g.child(
        Button::new(SharedString::from(format!("opt-{}-{i}", m.id)), o.clone()).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| {
          let rt = this.rt.clone();
          let (c, text) = (this.id.clone(), o2.clone());
          this.run(cx, async move { agent::api::chat::send(&rt, &c, &text, &[], None).await }, |_, _, _| {});
        })),
      );
    }
    card = card.child(g);
  }
  card.into_any_element()
}

pub fn reasoning(pane: &mut ChatPane, m: &Message, i: usize, text: &str, cx: &mut Context<ChatPane>) -> AnyElement {
  let key = format!("{}:{i}", m.id);
  let open = pane.open.contains(&key);
  guise::AIReasoning::new(SharedString::from(format!("reason-{key}")), text.to_string())
    .open(open)
    .on_toggle(cx.listener(move |this, _, _, cx| {
      if !this.open.remove(&key) {
        this.open.insert(key.clone());
      }
      cx.notify();
    }))
    .into_any_element()
}

pub fn handoff(from: &str, to: &str, direction: &str, cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  let text = match direction {
    "out" => crate::i18n::tf("Handed off to {}", &[to]),
    "transfer" => crate::i18n::tf("{} passed this task to you", &[from]),
    _ => crate::i18n::tf("Message from {}", &[from]),
  };
  div()
    .flex()
    .items_center()
    .gap(px(6.0))
    .text_size(px(12.0))
    .text_color(ink.dimmed)
    .child(guise::Icon::new(IconName::ArrowRightLeft).size(Size::Xs))
    .child(SharedString::from(text))
    .into_any_element()
}

pub fn routine(_pane: &mut ChatPane, id: &str, name: &str, event: &str, cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  let text = match event {
    "created" => crate::i18n::tf("Routine \"{}\" created", &[name]),
    "updated" => crate::i18n::tf("Routine \"{}\" updated", &[name]),
    "test" => crate::i18n::tf("Test run of \"{}\"", &[name]),
    _ => crate::i18n::tf("Routine run: \"{}\"", &[name]),
  };
  let _ = id;
  div()
    .id(SharedString::from(format!("routine-{id}-{event}")))
    .flex()
    .items_center()
    .gap(px(6.0))
    .px(px(10.0))
    .py(px(6.0))
    .rounded(px(8.0))
    .bg(ink.primary.opacity(0.08))
    .text_size(px(12.5))
    .cursor_pointer()
    .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::actions::Details), cx))
    .child(div().text_color(ink.primary).child(guise::Icon::new(IconName::CalendarClock).size(Size::Xs)))
    .child(SharedString::from(text))
    .into_any_element()
}

pub fn connect(plugin: &str, status: &str, cx: &mut Context<ChatPane>) -> AnyElement {
  let label = match status {
    "waiting" => t("Waiting for authorization"),
    "failed" => t("Sign-in failed"),
    "connected" => t("Connected"),
    _ => t("Connect"),
  };
  shell(cx)
    .child(div().font_weight(gpui::FontWeight::MEDIUM).child(crate::i18n::tf("Connect {}", &[plugin])))
    .child(
      Group::new()
        .child(Button::new("connect-open", label).size(Size::Xs).on_click(|_, w, cx| w.dispatch_action(Box::new(crate::actions::Marketplace), cx))),
    )
    .into_any_element()
}

pub fn error(_pane: &mut ChatPane, text: &str, cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  shell(cx)
    .border_color(ink.danger.opacity(0.4))
    .bg(ink.danger.opacity(0.06))
    .child(
      div()
        .flex()
        .gap(px(8.0))
        .items_center()
        .child(div().text_color(ink.danger).child(guise::Icon::new(IconName::CircleAlert).size(Size::Sm)))
        .child(div().text_size(px(13.0)).child(SharedString::from(text.to_string()))),
    )
    .child(
      Group::new().child(Button::new("retry", t("Try again")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, _, cx| {
        let rt = this.rt.clone();
        let c = this.id.clone();
        this.run(cx, async move { agent::api::chat::retry(&rt, &c).await }, |_, _, _| {});
      }))),
    )
    .into_any_element()
}
