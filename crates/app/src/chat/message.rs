//! One transcript entry: the user's bubble, a Bot's reply (markdown plus
//! cards), or an event line — with hover actions, reactions, and thread
//! replies.

use super::ChatPane;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString, Window};
use guise::{ActionIcon, IconName, Markdown, Size, Variant};
use store::Message;

pub const REACTIONS: [&str; 6] = ["👍", "❤️", "😂", "🎉", "👀", "✅"];

fn time(ms: i64, rt: &agent::Runtime) -> String {
  chrono::DateTime::from_timestamp_millis(ms)
    .map(|t| t.with_timezone(&rt.tz()).format("%-I:%M %p").to_string())
    .unwrap_or_default()
}

pub fn render(pane: &mut ChatPane, m: &Message, query: &str, window: &mut Window, cx: &mut Context<ChatPane>) -> AnyElement {
  let _ = query;
  match m.role.as_str() {
    "user" => user(pane, m, window, cx),
    "bot" => bot(pane, m, window, cx),
    _ => event(m, cx),
  }
}

fn actions(pane: &ChatPane, m: &Message, cx: &mut Context<ChatPane>) -> impl IntoElement {
  let ink = ink(cx);
  let id = m.id.clone();
  let body = m.body.clone();
  let group_id: SharedString = format!("acts-{}", m.id).into();
  let (id1, id2, id3) = (id.clone(), id.clone(), id.clone());
  let mut row = div()
    .flex()
    .items_center()
    .gap(px(2.0))
    .p(px(2.0))
    .rounded(px(8.0))
    .bg(ink.surface)
    .border_1()
    .border_color(ink.border)
    .child(
      ActionIcon::new(SharedString::from(format!("copy-{id}")), IconName::Copy)
        .size(Size::Xs)
        .variant(Variant::Subtle)
        .on_click(move |_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(body.clone()))),
    )
    .child(
      ActionIcon::new(SharedString::from(format!("thread-{id}")), IconName::MessageSquareReply)
        .size(Size::Xs)
        .variant(Variant::Subtle)
        .on_click(cx.listener(move |this, _, w, cx| this.open_thread(&id1, w, cx))),
    );
  for (i, e) in REACTIONS.iter().take(3).enumerate() {
    let mid = id2.clone();
    let emoji = e.to_string();
    row = row.child(
      div()
        .id(SharedString::from(format!("react-{i}-{mid}")))
        .px(px(4.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(ink.hover))
        .child(*e)
        .on_click(cx.listener(move |this, _, _, cx| {
          let rt = this.rt.clone();
          let (mid, emoji) = (mid.clone(), emoji.clone());
          this.run(cx, async move { store::messages::react(&rt.pool, &mid, &emoji).await }, |this, _, cx| this.load(cx));
        })),
    );
  }
  let more_id = id3.clone();
  row = row.child(
    div().id(SharedString::from(format!("moretip-{id3}"))).tooltip(guise::tooltip(t("More message actions"))).child(
      ActionIcon::new(SharedString::from(format!("more-{id3}")), IconName::Ellipsis)
        .size(Size::Xs)
        .variant(Variant::Subtle)
        .on_click(cx.listener(move |this, ev: &gpui::ClickEvent, w, cx| more(this, &more_id, ev.position(), w, cx))),
    ),
  );
  let _ = pane;
  div().id(group_id).absolute().top(px(-14.0)).right(px(8.0)).invisible().group_hover("msg", |s| s.visible()).child(row)
}

/// The "More message actions" menu (also on right-click).
pub fn more(pane: &mut ChatPane, id: &str, at: gpui::Point<gpui::Pixels>, window: &mut Window, cx: &mut Context<ChatPane>) {
  let Some(m) = pane.messages.iter().find(|m| m.id == id).cloned() else { return };
  let me = cx.entity().downgrade();
  let rid = m.id.clone();
  let mut menu = crate::menu::Menu::default();
  menu = menu.item(t("Copy request ID"), move |_, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(rid.clone())));
  let (me2, tid) = (me.clone(), m.id.clone());
  menu = menu.item(t("Reply in thread"), move |w, cx| {
    let _ = me2.update(cx, |p, cx| p.open_thread(&tid, w, cx));
  });
  for e in REACTIONS {
    let (me3, mid, emoji) = (me.clone(), m.id.clone(), e.to_string());
    menu = menu.item(crate::i18n::tf("React {}", &[e]), move |_, cx| {
      let _ = me3.update(cx, |p, cx| {
        let rt = p.rt.clone();
        let (mid, emoji) = (mid.clone(), emoji.clone());
        p.run(cx, async move { store::messages::react(&rt.pool, &mid, &emoji).await }, |p, _, cx| p.load(cx));
      });
    });
  }
  if m.role == "user" {
    let (me4, mid, body) = (me.clone(), m.id.clone(), m.body.clone());
    menu = menu.divider().item(t("Edit and resend"), move |_, cx| {
      let _ = me4.update(cx, |p, cx| {
        p.composer.update(cx, |c, cx| c.edit(&mid, &body, cx));
      });
    });
  }
  if m.role == "bot" {
    let me5 = me.clone();
    menu = menu.divider().item(t("Retry"), move |_, cx| {
      let _ = me5.update(cx, |p, cx| {
        let rt = p.rt.clone();
        let c = p.id.clone();
        p.run(cx, async move { agent::api::chat::retry(&rt, &c).await }, |_, _, _| {});
      });
    });
  }
  let (me6, mid) = (me.clone(), m.id.clone());
  menu = menu.danger(t("Delete message"), move |_, cx| {
    let _ = me6.update(cx, |p, cx| {
      let rt = p.rt.clone();
      let mid = mid.clone();
      p.run(cx, async move { store::messages::delete(&rt.pool, &mid).await }, |p, _, cx| p.load(cx));
    });
  });
  let entity = menu.show(at, 220.0, window, cx);
  pane.with_root(cx, |r, cx| {
    r.menu = entity;
    cx.notify();
  });
}

fn reactions(pane: &ChatPane, m: &Message, cx: &mut Context<ChatPane>) -> Option<AnyElement> {
  let r = m.reactions();
  let replies = pane.threads.get(&m.id).copied().unwrap_or(0);
  if r.is_empty() && replies == 0 {
    return None;
  }
  let ink = ink(cx);
  let mut row = div().flex().gap(px(6.0)).mt(px(6.0)).items_center();
  for (e, n) in r {
    let (mid, emoji) = (m.id.clone(), e.clone());
    row = row.child(
      div()
        .id(SharedString::from(format!("rx-{}-{e}", m.id)))
        .px(px(8.0))
        .py(px(2.0))
        .rounded_full()
        .bg(ink.surface)
        .border_1()
        .border_color(ink.border)
        .text_size(px(12.0))
        .cursor_pointer()
        .child(format!("{e} {n}"))
        .on_click(cx.listener(move |this, _, _, cx| {
          let rt = this.rt.clone();
          let (mid, emoji) = (mid.clone(), emoji.clone());
          this.run(cx, async move { store::messages::react(&rt.pool, &mid, &emoji).await }, |this, _, cx| this.load(cx));
        })),
    );
  }
  if replies > 0 {
    let mid = m.id.clone();
    row = row.child(
      div()
        .id(SharedString::from(format!("replies-{}", m.id)))
        .text_size(px(12.0))
        .text_color(ink.primary)
        .cursor_pointer()
        .child(crate::i18n::tf("{} replies", &[&replies.to_string()]))
        .on_click(cx.listener(move |this, _, w, cx| this.open_thread(&mid, w, cx))),
    );
  }
  Some(row.into_any_element())
}

fn user(pane: &mut ChatPane, m: &Message, window: &mut Window, cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  let parts = agent::part::parse(&m.parts);
  let mut bubble = div()
    .max_w(px(620.0))
    .px(px(14.0))
    .py(px(10.0))
    .rounded(px(14.0))
    .bg(ink.primary.opacity(0.14))
    .flex()
    .flex_col()
    .gap(px(8.0));
  if !m.body.is_empty() {
    bubble = bubble.child(Markdown::new(m.body.clone()).size(Size::Sm));
  }
  for (i, p) in parts.iter().enumerate() {
    bubble = bubble.child(crate::cards::render(pane, m, i, p, window, cx));
  }
  let mid = m.id.clone();
  div()
    .id(SharedString::from(format!("m-{}", m.id)))
    .group("msg")
    .relative()
    .w_full()
    .flex()
    .flex_col()
    .items_end()
    .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, ev: &gpui::MouseDownEvent, w, cx| more(this, &mid, ev.position, w, cx)))
    .child(bubble)
    .children(reactions(pane, m, cx))
    .child(div().text_size(px(11.0)).text_color(ink.dimmed).mt(px(2.0)).child(time(m.created, &pane.rt)))
    .child(actions(pane, m, cx))
    .into_any_element()
}

fn bot(pane: &mut ChatPane, m: &Message, window: &mut Window, cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  let author = m.bot_id.as_ref().and_then(|b| pane.bots.get(b)).cloned().unwrap_or_default();
  let streaming = m.status == "streaming";
  let body = pane.streams.get(&m.id).cloned().filter(|_| streaming).unwrap_or_else(|| m.body.clone());
  let parts = agent::part::parse(&m.parts);
  let mut content = div().flex().flex_col().gap(px(8.0)).min_w_0().flex_1();
  content = content.child(
    div()
      .flex()
      .items_center()
      .gap(px(8.0))
      .child(div().font_weight(gpui::FontWeight::SEMIBOLD).text_size(px(13.0)).child(author.name.clone()))
      .child(div().text_size(px(11.0)).text_color(ink.dimmed).child(time(m.created, &pane.rt)))
      .when(m.status == "stopped", |d| d.child(guise::Badge::new(t("Stopped")).size(Size::Xs).variant(Variant::Light)))
      .when(m.status == "error", |d| d.child(guise::Badge::new(t("Error")).size(Size::Xs).color(guise::ColorName::Red))),
  );
  let mut text_shown = false;
  for (i, p) in parts.iter().enumerate() {
    if let agent::Part::Reasoning { .. } = p {
      content = content.child(crate::cards::render(pane, m, i, p, window, cx));
    }
  }
  // Tool activity renders before the final text so the reply reads last.
  for (i, p) in parts.iter().enumerate() {
    if matches!(p, agent::Part::Tool { .. }) {
      content = content.child(crate::cards::render(pane, m, i, p, window, cx));
    }
  }
  if !body.trim().is_empty() {
    text_shown = true;
    content = content.child(Markdown::new(body.clone()).size(Size::Sm));
  }
  if streaming && !text_shown {
    content = content.child(guise::AIThinking::new().label(t("Working…")).size(Size::Sm));
  }
  for (i, p) in parts.iter().enumerate() {
    if !matches!(p, agent::Part::Tool { .. } | agent::Part::Reasoning { .. }) {
      content = content.child(crate::cards::render(pane, m, i, p, window, cx));
    }
  }
  if let Some(r) = reactions(pane, m, cx) {
    content = content.child(r);
  }
  let mid = m.id.clone();
  div()
    .id(SharedString::from(format!("m-{}", m.id)))
    .group("msg")
    .relative()
    .flex()
    .gap(px(12.0))
    .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, ev: &gpui::MouseDownEvent, w, cx| more(this, &mid, ev.position, w, cx)))
    .child(div().pt(px(2.0)).child(crate::avatar::face(&author, 30.0, cx)))
    .child(content)
    .child(actions(pane, m, cx))
    .into_any_element()
}

fn event(m: &Message, cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  div()
    .flex()
    .justify_center()
    .text_size(px(12.0))
    .text_color(ink.dimmed)
    .child(SharedString::from(m.body.clone()))
    .into_any_element()
}
