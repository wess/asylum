//! The conversation header: who you're talking to (click for details), and
//! the voice chat, computer, Teach a task, and details controls.

use super::ChatPane;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, Context, SharedString, Window};
use guise::{ActionIcon, IconName, Size, Variant};

pub fn render(pane: &mut ChatPane, _window: &mut Window, cx: &mut Context<ChatPane>) -> impl IntoElement {
  let ink = ink(cx);
  let group = pane.is_group();
  let (face, name, sub): (gpui::AnyElement, SharedString, SharedString) = match (group, pane.bot().cloned()) {
    (false, Some(b)) => {
      let status = match b.status.as_str() {
        "working" => t("Working…").to_string(),
        "waiting" => t("Waiting for you").to_string(),
        _ if !b.provider.is_empty() => {
          let pin = if b.model.is_empty() { b.provider.clone() } else { format!("{} · {}", b.provider, b.model) };
          if b.label.is_empty() { pin } else { format!("{} — {pin}", b.label) }
        }
        _ => b.label.clone(),
      };
      (crate::avatar::avatar(&b, 28.0, cx), b.name.clone().into(), status.into())
    }
    _ => {
      let c = pane.chat.clone().unwrap_or_default();
      let names: Vec<String> = pane.members.iter().filter_map(|m| pane.bots.get(m).map(|b| b.name.clone())).collect();
      let face = div()
        .flex()
        .children(pane.members.iter().take(3).filter_map(|m| pane.bots.get(m)).map(|b| div().ml(px(-6.0)).child(crate::avatar::face(b, 22.0, cx))))
        .into_any_element();
      (face, c.title.clone().into(), names.join(", ").into())
    }
  };
  let busy = pane.busy();
  let in_call = pane.root.upgrade().and_then(|r| r.read(cx).voice.clone()).is_some();
  let recording = pane.bot().is_some_and(|b| pane.root.upgrade().is_some_and(|r| r.read(cx).panel.as_ref().is_some_and(|p| p.read(cx).recording && p.read(cx).bot == b.id)));
  let computer_open = pane.root.upgrade().is_some_and(|r| r.read(cx).right == crate::root::Right::Computer);

  let mut right = div().flex().items_center().gap(px(4.0));
  if busy {
    right = right.child(
      guise::Button::new("stop", t("Stop"))
        .size(Size::Xs)
        .variant(Variant::Light)
        .color(guise::ColorName::Red)
        .left_section(guise::Icon::new(IconName::Square).size(Size::Xs))
        .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::actions::StopBot), cx)),
    );
  }
  if !group && computer_open {
    right = right.child(
      guise::Button::new("teach", if recording { t("Stop") } else { t("Teach a task") })
        .size(Size::Xs)
        .variant(if recording { Variant::Filled } else { Variant::Default })
        .color(if recording { guise::ColorName::Red } else { guise::ColorName::Violet })
        .left_section(guise::Icon::new(if recording { IconName::CircleStop } else { IconName::GraduationCap }).size(Size::Xs))
        .on_click(cx.listener(move |this, _, w, cx| {
          let root = this.root.clone();
          let _ = root.update(cx, |r, cx| {
            if let Some(p) = r.panel.clone() {
              p.update(cx, |p, cx| p.toggle_teach(w, cx));
            }
          });
        })),
    );
  }
  if pane.rt.settings().voice_enabled && !group && !in_call && pane.bot().is_some_and(|b| !(b.is_team() && b.owner != pane.rt.settings().user_name && !b.owner.is_empty())) {
    right = right.child(
      ActionIcon::new("voice", IconName::AudioLines)
        .variant(Variant::Subtle)
        .on_click(cx.listener(|this, _, w, cx| crate::voicechat::start(this, w, cx))),
    );
  }
  if !group {
    right = right.child(
      ActionIcon::new("computer", IconName::Monitor)
        .variant(if computer_open { Variant::Light } else { Variant::Subtle })
        .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::actions::OpenComputer), cx)),
    );
  }
  right = right
    .child(ActionIcon::new("find", IconName::Search).variant(Variant::Subtle).on_click(|_, w, cx| w.dispatch_action(Box::new(crate::actions::FindInChat), cx)))
    .child(ActionIcon::new("more", IconName::Ellipsis).variant(Variant::Subtle).on_click(cx.listener(|this, ev: &gpui::ClickEvent, w, cx| overflow(this, ev.position(), w, cx))))
    .child(ActionIcon::new("info", IconName::PanelRight).variant(Variant::Subtle).on_click(|_, w, cx| w.dispatch_action(Box::new(crate::actions::Details), cx)));

  div()
    .h(px(56.0))
    .flex_none()
    .flex()
    .items_center()
    .justify_between()
    .px(px(20.0))
    .border_b_1()
    .border_color(ink.border)
    .child(
      div()
        .id("chat-title")
        .flex()
        .items_center()
        .gap(px(10.0))
        .cursor_pointer()
        .on_click(|_, w, cx| w.dispatch_action(Box::new(crate::actions::Details), cx))
        .child(face)
        .child(
          div()
            .flex()
            .flex_col()
            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(name))
            .when(!sub.is_empty(), |d| d.child(div().text_size(px(12.0)).text_color(ink.dimmed).truncate().max_w(px(420.0)).child(sub))),
        ),
    )
    .child(right)
}

/// The conversation's overflow menu.
fn overflow(pane: &mut ChatPane, at: gpui::Point<gpui::Pixels>, window: &mut Window, cx: &mut Context<ChatPane>) {
  let root = pane.root.clone();
  let chat = pane.id.clone();
  let unread = pane.chat.as_ref().is_some_and(|c| c.unread);
  let bot = pane.bot().cloned().filter(|_| !pane.is_group());
  let mut m = crate::menu::Menu::default();
  let r1 = root.clone();
  m = m.item(t("Agent settings"), move |w, cx| {
    let _ = r1.update(cx, |r, cx| r.show_details(w, cx));
  });
  let (r2, c2) = (root.clone(), chat.clone());
  m = m.item(if unread { t("Mark as Read") } else { t("Mark as Unread") }, move |_, cx| {
    let _ = r2.update(cx, |r, cx| {
      let rt = r.rt.clone();
      let c = c2.clone();
      r.run(cx, async move { store::chats::set_unread(&rt.pool, &c, !unread).await }, |r, _, cx| r.reload(cx));
    });
  });
  let c3 = chat.clone();
  m = m.item(t("Copy conversation ID"), move |_, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(c3.clone())));
  if let Some(b) = bot {
    let r4 = root.clone();
    m = m.divider().section(t("Share")).item(t("Create template"), move |w, cx| {
      let _ = r4.update(cx, |r, cx| {
        r.show_details(w, cx);
        if let Some(d) = r.details.clone() {
          d.update(cx, |d, cx| {
            d.tab = crate::details::Tab::Share;
            cx.notify();
          });
        }
      });
    });
    let r5 = root.clone();
    if !b.is_team() {
      m = m.item(t("Publish to Team"), move |w, cx| {
        let _ = r5.update(cx, |r, cx| {
          r.show_details(w, cx);
          if let Some(d) = r.details.clone() {
            d.update(cx, |d, cx| {
              d.tab = crate::details::Tab::Share;
              cx.notify();
            });
          }
        });
      });
    }
    let (r6, id6) = (root.clone(), b.id.clone());
    m = m.divider().item(t("Duplicate"), move |_, cx| {
      let _ = r6.update(cx, |r, cx| {
        let rt = r.rt.clone();
        let id = id6.clone();
        r.run(cx, async move { agent::api::bots::duplicate(&rt, &id).await }, |r, (_, c), cx| {
          r.reload(cx);
          r.pending_open(c.id, cx);
        });
      });
    });
    let (r7, id7, name) = (root.clone(), b.id.clone(), b.name.clone());
    m = m.danger(t("Delete"), move |w, cx| {
      let (id, name) = (id7.clone(), name.clone());
      let _ = r7.update(cx, |r, cx| {
        crate::root::dialogs::confirm(r, crate::i18n::tf("Delete {}?", &[&name]), t("This removes the Agent, its conversation, and its routines. Files on the computer and browser sign-ins stay."), t("Delete"), w, cx, move |r, _, cx| {
          let rt = r.rt.clone();
          let id = id.clone();
          r.active = None;
          r.pane = None;
          r.close_right(cx);
          r.run(cx, async move { agent::api::bots::delete(&rt, &id).await }, |r, _, cx| r.reload(cx));
        });
      });
    });
  }
  let entity = m.show(at, 220.0, window, cx);
  pane.with_root(cx, |r, cx| {
    r.menu = entity;
    cx.notify();
  });
}
