//! Teach a task: name it, describe the result, then do the work on the
//! Bot's screen while it records (up to ten minutes, no audio). Stopping
//! drafts a skill in the chat for review.

use super::Panel;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, Entity, Window};
use guise::{Button, Group, Size, TextInput, Variant};

pub struct Form {
  pub name: Entity<TextInput>,
  pub result: Entity<TextInput>,
}

impl Form {
  pub fn new(window: &mut Window, cx: &mut Context<Panel>) -> Self {
    let name = cx.new(|cx| TextInput::new(cx).placeholder(t("Short name, e.g. File an expense")).size(Size::Sm));
    let result = cx.new(|cx| TextInput::new(cx).placeholder(t("What should the result be?")).size(Size::Sm));
    window.focus(&name.read(cx).focus_handle(), cx);
    Self { name, result }
  }
}

pub fn form(f: &Form, cx: &mut Context<Panel>) -> AnyElement {
  let ink = ink(cx);
  div()
    .flex()
    .flex_col()
    .gap(px(6.0))
    .p(px(10.0))
    .rounded(px(8.0))
    .bg(ink.surface)
    .child(div().font_weight(gpui::FontWeight::SEMIBOLD).text_size(px(13.0)).child(t("Teach a task")))
    .child(f.name.clone())
    .child(f.result.clone())
    .child(div().text_size(px(11.5)).text_color(ink.dimmed).child(t("Do the task on the screen below. Recording lasts up to 10 minutes and captures no audio. Don't type passwords while recording.")))
    .child(
      Group::new()
        .gap(Size::Xs)
        .child(Button::new("start-teach", t("Start recording")).size(Size::Xs).color(guise::ColorName::Red).on_click(cx.listener(|this, _, _, cx| start(this, cx))))
        .child(Button::new("cancel-teach", t("Cancel")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(|this, _, _, cx| {
          this.teach = None;
          cx.notify();
        }))),
    )
    .into_any_element()
}

fn start(p: &mut Panel, cx: &mut Context<Panel>) {
  let Some(f) = &p.teach else { return };
  let name = f.name.read(cx).text();
  let result = f.result.read(cx).text();
  if name.trim().is_empty() {
    p.status = Some(t("Give the task a short name.").into());
    cx.notify();
    return;
  }
  let (rt, bot) = (p.rt.clone(), p.bot.clone());
  let chat = p.root.upgrade().and_then(|r| r.read(cx).active.clone()).unwrap_or_default();
  if chat.is_empty() {
    return;
  }
  cx.spawn(async move |this, cx| {
    let r = crate::tk::run(async move { agent::teach::start(&rt, &bot, &chat, &name, &result).await }).await;
    let _ = this.update(cx, |p, cx| {
      match r {
        Ok(()) => {
          p.teach = None;
          p.recording = true;
          p.started = Some(std::time::Instant::now());
          p.taking = true;
        }
        Err(e) => p.status = Some(e.to_string()),
      }
      cx.notify();
    });
  })
  .detach();
}

pub fn stop(p: &mut Panel, cx: &mut Context<Panel>) {
  p.recording = false;
  p.taking = false;
  p.started = None;
  p.status = Some(t("Drafting a skill from your demo…").into());
  let (rt, bot) = (p.rt.clone(), p.bot.clone());
  cx.spawn(async move |this, cx| {
    let r = crate::tk::run(async move { agent::teach::stop(&rt, &bot).await }).await;
    let _ = this.update(cx, |p, cx| {
      p.status = Some(match r {
        Ok(()) => t("Skill drafted. Review it in the chat.").into(),
        Err(e) => e.to_string(),
      });
      cx.notify();
    });
  })
  .detach();
  cx.notify();
}

pub fn banner(p: &Panel, cx: &mut Context<Panel>) -> AnyElement {
  let ink = ink(cx);
  let secs = p.started.map(|s| s.elapsed().as_secs()).unwrap_or(0);
  div()
    .flex()
    .items_center()
    .gap(px(8.0))
    .p(px(8.0))
    .rounded(px(8.0))
    .bg(ink.danger.opacity(0.12))
    .child(div().size(px(8.0)).rounded_full().bg(ink.danger))
    .child(div().flex_1().text_size(px(12.5)).child(format!("{} {}:{:02} / 10:00", t("Recording"), secs / 60, secs % 60)))
    .child(Button::new("stop-teach", t("Stop")).size(Size::Xs).color(guise::ColorName::Red).on_click(cx.listener(|this, _, _, cx| stop(this, cx))))
    .into_any_element()
}
