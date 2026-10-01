use crate::i18n::t;
use crate::root::Root;
use crate::state::Item;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString, Window};
use guise::{IconName, Size};

pub fn render(root: &mut Root, item: &Item, index: usize, _window: &mut Window, cx: &mut Context<Root>) -> AnyElement {
  let ink = ink(cx);
  let compact = root.compact;
  let chat = root.snap.chat_for(item).cloned();
  let active = chat.as_ref().is_some_and(|c| root.active.as_deref() == Some(c.id.as_str()));
  let attention = chat.as_ref().is_some_and(|c| c.attention) || root.snap.approvals.iter().any(|a| chat.as_ref().is_some_and(|c| a.chat_id.as_deref() == Some(c.id.as_str())));
  let unread = chat.as_ref().is_some_and(|c| c.unread) && !active;
  let in_call = root.voice.as_ref().is_some_and(|v| chat.as_ref().is_some_and(|c| v.read(cx).chat == c.id));

  let (face, name, sub, star): (AnyElement, SharedString, Option<SharedString>, bool) = match item {
    Item::Bot(id) => {
      let Some(b) = root.snap.bot(id).cloned() else { return div().into_any_element() };
      let working = b.status == "working";
      let sub = if in_call {
        Some(t("Voice chat in progress").into())
      } else if working {
        Some(t("Working…").into())
      } else if b.status == "waiting" {
        Some(t("Needs attention").into())
      } else if b.is_team() && !b.owner.is_empty() {
        Some(b.owner.clone().into())
      } else if !b.label.is_empty() {
        Some(b.label.clone().into())
      } else {
        None
      };
      (crate::avatar::avatar(&b, if compact { 30.0 } else { 26.0 }, cx), b.name.clone().into(), sub, b.primary_bot)
    }
    Item::Group(id) => {
      let g = root.snap.groups.iter().find(|c| &c.id == id).cloned().unwrap_or_default();
      let face = div()
        .size(px(32.0))
        .flex_none()
        .rounded(px(8.0))
        .bg(ink.hover)
        .flex()
        .items_center()
        .justify_center()
        .text_color(ink.dimmed)
        .child(guise::Icon::new(IconName::Users).size(Size::Sm))
        .into_any_element();
      (face, g.title.clone().into(), None, false)
    }
  };

  let renaming = root.rename.as_ref().filter(|(it, _)| it == item).map(|(_, input)| input.clone());
  let it = item.clone();
  let it2 = item.clone();
  let it3 = item.clone();
  let key: SharedString = match item {
    Item::Bot(b) => format!("row-b-{b}").into(),
    Item::Group(g) => format!("row-g-{g}").into(),
  };
  let mut row = div()
    .id(key)
    .flex()
    .items_center()
    .gap(px(10.0))
    .px(px(8.0))
    .py(px(6.0))
    .rounded(px(8.0))
    .cursor_pointer()
    .when(active, |d| d.bg(ink.hover))
    .hover(|s| s.bg(ink.hover))
    .when(compact, |d| d.justify_center().px(px(0.0)))
    .tooltip(guise::tooltip(name.clone()))
    .on_click(cx.listener(move |this, ev: &gpui::ClickEvent, w, cx| {
      if ev.click_count() >= 2 && !this.compact {
        crate::sidebar::menu::start_rename(this, &it, w, cx);
      } else {
        this.open_item(&it, w, cx);
      }
    }))
    .on_mouse_down(
      gpui::MouseButton::Right,
      cx.listener(move |this, ev: &gpui::MouseDownEvent, w, cx| crate::sidebar::menu::item(this, &it2, ev.position, w, cx)),
    )
    .child(face);
  if compact {
    if attention || unread {
      row = row.child(div().absolute().top(px(4.0)).right(px(8.0)).size(px(8.0)).rounded_full().bg(if attention { ink.warning } else { ink.primary }));
    }
    return row.relative().into_any_element();
  }
  let text = match renaming {
    Some(input) => div().flex_1().child(input).into_any_element(),
    None => div()
      .flex_1()
      .min_w_0()
      .flex()
      .flex_col()
      .child(
        div()
          .flex()
          .items_center()
          .gap(px(4.0))
          .child(div().truncate().font_weight(if unread { gpui::FontWeight::SEMIBOLD } else { gpui::FontWeight::NORMAL }).child(name))
          .when(star, |d| d.child(div().text_color(ink.warning).child(guise::Icon::new(IconName::Star).size(Size::Xs)))),
      )
      .when_some(sub, |d, s| d.child(div().truncate().text_size(px(11.5)).text_color(if attention { ink.warning } else { ink.dimmed }).child(s)))
      .into_any_element(),
  };
  row = row.child(text);
  if attention {
    row = row.child(div().id(SharedString::from(format!("att-{index}"))).size(px(8.0)).rounded_full().bg(ink.warning).tooltip(guise::tooltip(t("Needs attention"))));
  } else if unread {
    row = row.child(div().size(px(8.0)).rounded_full().bg(ink.primary));
  }
  if index < 9 && !active {
    let _ = it3;
  }
  row.into_any_element()
}

/// "You built an Agent. Now your whole team gets a teammate." — shown for a
/// Team Bot that is ready but not yet published.
pub fn team_card(root: &mut Root, compact: bool, cx: &mut Context<Root>) -> Option<AnyElement> {
  if compact {
    return None;
  }
  let ink = ink(cx);
  let b = root.snap.bots.iter().find(|b| b.is_team() && !b.published && agent::api::team::ready(b).is_ok())?.clone();
  let id = b.id.clone();
  Some(
    div()
      .mx(px(8.0))
      .mb(px(6.0))
      .p(px(10.0))
      .rounded(px(10.0))
      .bg(ink.primary.opacity(0.12))
      .flex()
      .flex_col()
      .gap(px(6.0))
      .child(div().text_size(px(12.5)).child(t("You built an Agent. Now your whole team gets a teammate.")))
      .child(
        guise::Button::new("publish-team", t("Publish to team")).size(Size::Xs).on_click(cx.listener(move |this, _, _, cx| {
          let rt = this.rt.clone();
          let id = id.clone();
          this.run(cx, async move { agent::api::team::publish(&rt, &id, true).await }, |this, link, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(link));
            this.toast(t("Published. Team link copied."), cx);
          });
        })),
      )
      .into_any_element(),
  )
}
