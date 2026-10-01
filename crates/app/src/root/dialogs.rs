//! Small in-window dialogs: a one-field prompt and a confirmation.

use super::Root;
use crate::i18n::t;
use gpui::prelude::*;
use gpui::{div, Context, Entity, SharedString, Subscription, Window};
use guise::{Button, Group, Modal, Size, Stack, TextInput, TextInputEvent, Variant};

type OnText = Box<dyn Fn(&mut Root, String, &mut Window, &mut Context<Root>)>;

pub struct Prompt {
  root: gpui::WeakEntity<Root>,
  title: SharedString,
  input: Entity<TextInput>,
  on: std::rc::Rc<OnText>,
  _sub: Subscription,
}

impl Prompt {
  fn submit(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
    let on = self.on.clone();
    let _ = self.root.update(cx, |root, cx| {
      root.close_modal(window, cx);
      if !text.trim().is_empty() {
        on(root, text.trim().to_string(), window, cx);
      }
    });
  }
}

impl Render for Prompt {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let root = self.root.clone();
    let root2 = self.root.clone();
    let input = self.input.clone();
    div().absolute().top_0().left_0().size_full().child(
      Modal::new()
        .title(self.title.clone())
        .width(420.0)
        .on_close(move |_, w, cx| {
          let _ = root.update(cx, |r, cx| r.close_modal(w, cx));
        })
        .child(
          Stack::new()
            .gap(Size::Md)
            .child(self.input.clone())
            .child(
              Group::new()
                .justify(guise::Justify::End)
                .child(Button::new("cancel", t("Cancel")).variant(Variant::Default).on_click(move |_, w, cx| {
                  let _ = root2.update(cx, |r, cx| r.close_modal(w, cx));
                }))
                .child(Button::new("ok", t("Save")).on_click(cx.listener(move |this, _, w, cx| {
                  let text = input.read(cx).text();
                  this.submit(text, w, cx);
                }))),
            ),
        ),
    )
  }
}

pub fn prompt(
  root: &mut Root,
  title: impl Into<SharedString>,
  placeholder: impl Into<SharedString>,
  initial: &str,
  window: &mut Window,
  cx: &mut Context<Root>,
  on: impl Fn(&mut Root, String, &mut Window, &mut Context<Root>) + 'static,
) {
  let weak = cx.entity().downgrade();
  let title = title.into();
  let placeholder: SharedString = placeholder.into();
  let initial = initial.to_string();
  let view = cx.new(|cx| {
    let input = cx.new(|cx| TextInput::new(cx).value(&initial).placeholder(placeholder.clone()));
    let sub = cx.subscribe_in(&input, window, |this: &mut Prompt, _, ev: &TextInputEvent, w, cx| {
      if let TextInputEvent::Submit(text) = ev {
        this.submit(text.clone(), w, cx);
      }
    });
    window.focus(&input.read(cx).focus_handle(), cx);
    Prompt { root: weak, title, input, on: std::rc::Rc::new(Box::new(on)), _sub: sub }
  });
  root.set_modal(view, cx);
}

type OnConfirm = std::rc::Rc<Box<dyn Fn(&mut Root, &mut Window, &mut Context<Root>)>>;

pub struct Confirm {
  root: gpui::WeakEntity<Root>,
  title: SharedString,
  message: SharedString,
  label: SharedString,
  on: OnConfirm,
}

impl Render for Confirm {
  fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    let (r1, r2) = (self.root.clone(), self.root.clone());
    let on = self.on.clone();
    div().absolute().top_0().left_0().size_full().child(
      guise::ConfirmModal::new()
        .title(self.title.clone())
        .message(self.message.clone())
        .confirm_label(self.label.clone())
        .cancel_label(t("Cancel"))
        .danger()
        .width(440.0)
        .on_cancel(move |_, w, cx| {
          let _ = r1.update(cx, |r, cx| r.close_modal(w, cx));
        })
        .on_confirm(move |_, w, cx| {
          let on = on.clone();
          let _ = r2.update(cx, |r, cx| {
            r.close_modal(w, cx);
            on(r, w, cx);
          });
        }),
    )
  }
}

pub fn confirm(
  root: &mut Root,
  title: impl Into<SharedString>,
  message: impl Into<SharedString>,
  label: impl Into<SharedString>,
  _window: &mut Window,
  cx: &mut Context<Root>,
  on: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static,
) {
  let weak = cx.entity().downgrade();
  let (title, message, label) = (title.into(), message.into(), label.into());
  let view = cx.new(|_| Confirm { root: weak, title, message, label, on: std::rc::Rc::new(Box::new(on)) });
  root.set_modal(view, cx);
}
