//! A thread: one message and the replies to it, with its own composer.

use super::ChatPane;
use crate::composer::Composer;
use crate::i18n::t;
use crate::theme::ink;
use crate::tk;
use agent::{Event, Runtime};
use gpui::prelude::*;
use gpui::{div, px, Context, Entity, WeakEntity, Window};
use guise::{ActionIcon, IconName, Markdown, Size, Variant};
use store::Message;

pub struct Thread {
  rt: Runtime,
  chat: String,
  root: String,
  pane: WeakEntity<ChatPane>,
  head: Option<Message>,
  replies: Vec<Message>,
  composer: Entity<Composer>,
}

impl Thread {
  pub fn new(rt: Runtime, chat: String, root: String, pane: WeakEntity<ChatPane>, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let composer = cx.new(|cx| Composer::new(rt.clone(), chat.clone(), Some(root.clone()), pane.clone(), window, cx));
    let mut t = Self { rt, chat, root, pane, head: None, replies: Vec::new(), composer };
    t.load(cx);
    t
  }

  fn load(&mut self, cx: &mut Context<Self>) {
    let rt = self.rt.clone();
    let root = self.root.clone();
    cx.spawn(async move |this, cx| {
      let r = tk::run(async move { Ok((store::messages::get(&rt.pool, &root).await?, store::messages::thread(&rt.pool, &root).await?)) }).await;
      let _ = this.update(cx, |this, cx| {
        if let Ok((h, r)) = r {
          this.head = Some(h);
          this.replies = r;
        }
        cx.notify();
      });
    })
    .detach();
  }

  pub fn on_event(&mut self, e: &Event, cx: &mut Context<Self>) {
    if let Event::Message { chat, .. } = e {
      if chat == &self.chat {
        self.load(cx);
      }
    }
  }
}

impl Render for Thread {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let pane = self.pane.upgrade();
    let name = |m: &Message| -> String {
      match &m.bot_id {
        Some(b) => pane.as_ref().and_then(|p| p.read(cx).bots.get(b).map(|b| b.name.clone())).unwrap_or_default(),
        None => t("You").into(),
      }
    };
    let mut list = div().id("thread-list").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap(px(12.0)).p(px(16.0));
    let all: Vec<Message> = self.head.iter().cloned().chain(self.replies.iter().cloned()).collect();
    for (i, m) in all.iter().enumerate() {
      list = list.child(
        div()
          .flex()
          .flex_col()
          .gap(px(4.0))
          .when(i == 0, |d| d.pb(px(10.0)).border_b_1().border_color(ink.border))
          .child(div().text_size(px(12.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(name(m)))
          .child(Markdown::new(m.body.clone()).size(Size::Sm)),
      );
    }
    div()
      .size_full()
      .flex()
      .flex_col()
      .bg(ink.body)
      .child(
        div()
          .h(px(56.0))
          .flex()
          .items_center()
          .justify_between()
          .px(px(16.0))
          .border_b_1()
          .border_color(ink.border)
          .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(t("Thread")))
          .child(ActionIcon::new("close-thread", IconName::X).variant(Variant::Subtle).on_click(cx.listener(|this, _, _, cx| {
            let _ = this.pane.update(cx, |p, cx| {
              p.thread = None;
              cx.notify();
            });
          }))),
      )
      .child(list)
      .child(div().p(px(12.0)).child(self.composer.clone()))
  }
}
