//! Find in this chat: filters the transcript to matching messages and
//! highlights the query.

use super::ChatPane;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, App, Context, Entity, Window};
use guise::{ActionIcon, IconName, TextInput, Variant};

pub struct Find {
  pub input: Entity<TextInput>,
}

impl Find {
  pub fn new(window: &mut Window, cx: &mut Context<ChatPane>) -> Self {
    let input = cx.new(|cx| TextInput::new(cx).placeholder(t("Find in this chat")).size(guise::Size::Sm));
    window.focus(&input.read(cx).focus_handle(), cx);
    let sub = cx.observe(&input, |_, _, cx| cx.notify());
    sub.detach();
    Self { input }
  }

  pub fn query(&self, cx: &App) -> String {
    self.input.read(cx).text()
  }

  pub fn render(&self, pane: &ChatPane, cx: &mut Context<ChatPane>) -> impl IntoElement {
    let ink = ink(cx);
    let q = self.query(cx).to_lowercase();
    let hits = if q.is_empty() { 0 } else { pane.messages.iter().filter(|m| m.body.to_lowercase().contains(&q)).count() };
    div()
      .flex()
      .items_center()
      .gap(px(8.0))
      .px(px(20.0))
      .py(px(8.0))
      .border_b_1()
      .border_color(ink.border)
      .child(div().flex_1().child(self.input.clone()))
      .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(crate::i18n::tf("{} matches", &[&hits.to_string()])))
      .child(ActionIcon::new("close-find", IconName::X).variant(Variant::Subtle).on_click(cx.listener(|this, _, _, cx| {
        this.find = None;
        cx.notify();
      })))
  }
}
