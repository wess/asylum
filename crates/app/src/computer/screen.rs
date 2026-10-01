//! The Bot's screen: a live preview of its browser page, with a URL bar and
//! navigation. On takeover the preview becomes interactive: clicks, scrolls,
//! and keys go to the page; the Bot never sees what you type.

use super::Panel;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{canvas, div, linear_color_stop, linear_gradient, px, AnyElement, Context, Hsla, KeyDownEvent, ObjectFit, Window};
use guise::{ActionIcon, Button, IconName, Size, Variant};

pub const PAGE_W: f32 = computer::browser::WIDTH as f32;
pub const PAGE_H: f32 = computer::browser::HEIGHT as f32;

/// The wallpaper follows the day: light in the morning, dark at night.
pub fn wallpaper(hour: u32) -> (Hsla, Hsla) {
  let c = |h: &str| guise::Color::hex(h).hsla();
  match hour {
    5..=9 => (c("#ffd6a5"), c("#bde0fe")),
    10..=15 => (c("#a2d2ff"), c("#cdb4db")),
    16..=19 => (c("#f4a261"), c("#6d597a")),
    _ => (c("#1b1b3a"), c("#3a2e5c")),
  }
}

fn named(key: &str) -> Option<&'static str> {
  Some(match key {
    "enter" => "Enter",
    "tab" => "Tab",
    "escape" => "Escape",
    "backspace" => "Backspace",
    "delete" => "Delete",
    "up" => "ArrowUp",
    "down" => "ArrowDown",
    "left" => "ArrowLeft",
    "right" => "ArrowRight",
    "pageup" => "PageUp",
    "pagedown" => "PageDown",
    "home" => "Home",
    "end" => "End",
    _ => return None,
  })
}

fn key(p: &mut Panel, ev: &KeyDownEvent, cx: &mut Context<Panel>) {
  if !p.taking {
    return;
  }
  let ks = &ev.keystroke;
  let (rt, bot) = (p.rt.clone(), p.bot.clone());
  if ks.modifiers.platform && ks.key == "v" {
    let text = cx.read_from_clipboard().and_then(|c| c.text()).unwrap_or_default();
    crate::tk::spawn(async move { rt.browser.keystroke(&bot, Some(&text), None).await });
    return;
  }
  if ks.modifiers.platform || ks.modifiers.control {
    return;
  }
  if let Some(n) = named(&ks.key) {
    crate::tk::spawn(async move { rt.browser.keystroke(&bot, None, Some(n)).await });
  } else if let Some(ch) = ks.key_char.clone() {
    crate::tk::spawn(async move { rt.browser.keystroke(&bot, Some(&ch), None).await });
  }
  cx.stop_propagation();
}

fn to_page(p: &Panel, pos: gpui::Point<gpui::Pixels>) -> Option<(f64, f64)> {
  let b = p.bounds.get()?;
  let x = (pos.x - b.origin.x) / b.size.width;
  let y = (pos.y - b.origin.y) / b.size.height;
  ((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)).then_some(((x * PAGE_W) as f64, (y * PAGE_H) as f64))
}

pub fn render(p: &mut Panel, window: &mut Window, cx: &mut Context<Panel>) -> AnyElement {
  let ink = ink(cx);
  let hour = chrono::Timelike::hour(&chrono::Utc::now().with_timezone(&p.rt.tz()));
  let (a, b) = wallpaper(hour);
  let wide = p.root.upgrade().is_some_and(|r| r.read(cx).wide);
  let width = if wide { (f32::from(window.viewport_size().width) - 340.0).clamp(488.0, PAGE_W) } else { 488.0 };
  let height = width * PAGE_H / PAGE_W;
  let bounds = p.bounds.clone();
  let mut frame = div()
    .id("screen")
    .relative()
    .w(px(width))
    .h(px(height))
    .rounded(px(10.0))
    .overflow_hidden()
    .border_2()
    .border_color(if p.taking { ink.warning } else if p.recording { ink.danger } else { ink.border })
    .bg(linear_gradient(160.0, linear_color_stop(a, 0.0), linear_color_stop(b, 1.0)))
    .track_focus(&p.focus)
    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| key(this, ev, cx)))
    .child(canvas(move |b, _, _| bounds.set(Some(b)), |_, _, _, _| {}).absolute().size_full());
  match &p.shot {
    Some(img) => frame = frame.child(gpui::img(img.clone()).size_full().object_fit(ObjectFit::Fill)),
    None => {
      frame = frame.child(
        div()
          .absolute()
          .size_full()
          .flex()
          .flex_col()
          .items_center()
          .justify_center()
          .gap(px(6.0))
          .text_color(gpui::white())
          .child(guise::Icon::new(IconName::Monitor).size(Size::Lg))
          .child(div().text_size(px(12.5)).child(t("The Bot's screen appears when it opens a website."))),
      )
    }
  }
  if p.taking {
    frame = frame
      .cursor_pointer()
      .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, ev: &gpui::MouseDownEvent, w, cx| {
        w.focus(&this.focus, cx);
        if let Some((x, y)) = to_page(this, ev.position) {
          let (rt, bot) = (this.rt.clone(), this.bot.clone());
          crate::tk::spawn(async move { rt.browser.pointer(&bot, x, y).await });
        }
      }))
      .on_scroll_wheel(cx.listener(|this, ev: &gpui::ScrollWheelEvent, _, _| {
        if let Some((x, y)) = to_page(this, ev.position) {
          let dy = -f32::from(ev.delta.pixel_delta(px(16.0)).y) as f64 * 2.0;
          let (rt, bot) = (this.rt.clone(), this.bot.clone());
          crate::tk::spawn(async move { rt.browser.wheel(&bot, x, y, dy).await });
        }
      }));
  }
  let mut col = div().flex().flex_col().gap(px(8.0));
  col = col.child(
    div()
      .flex()
      .items_center()
      .gap(px(4.0))
      .child(ActionIcon::new("nav-back", IconName::ArrowLeft).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(|this, _, _, cx| this.browse(|rt, bot| Box::pin(async move { rt.browser.back(&bot).await.map(|_| ()) }), cx))))
      .child(ActionIcon::new("nav-fwd", IconName::ArrowRight).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(|this, _, _, cx| this.browse(|rt, bot| Box::pin(async move { rt.browser.forward(&bot).await.map(|_| ()) }), cx))))
      .child(ActionIcon::new("nav-reload", IconName::RotateCw).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(|this, _, _, cx| this.browse(|rt, bot| Box::pin(async move { rt.browser.reload(&bot).await }), cx))))
      .child(div().flex_1().child(p.url.clone()))
      .child(ActionIcon::new("wide", if wide { IconName::Minimize2 } else { IconName::Maximize2 }).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(|this, _, _, cx| {
        let _ = this.root.update(cx, |r, cx| {
          r.wide = !r.wide;
          cx.notify();
        });
      }))),
  );
  if !p.page.is_empty() {
    col = col.child(div().text_size(px(11.5)).text_color(ink.dimmed).truncate().child(p.page.clone()));
  }
  if let Some(form) = &p.teach {
    col = col.child(super::teach::form(form, cx));
  }
  if p.recording {
    col = col.child(super::teach::banner(p, cx));
  }
  col = col.child(frame);
  col = col.child(if p.taking {
    div()
      .flex()
      .items_center()
      .gap(px(8.0))
      .child(div().flex_1().text_size(px(12.5)).child(t("You're in control. Click, scroll, and type on the screen. Your Bot never sees what you type.")))
      .child(Button::new("window", if p.rt.browser.is_headful() { t("Back to preview") } else { t("Open in a window") }).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(|this, _, _, cx| {
        let on = !this.rt.browser.is_headful();
        let rt = this.rt.clone();
        this.status = Some(if on { t("Opening a browser window for passkeys and security keys…").into() } else { t("Returning to the preview…").into() });
        crate::tk::spawn(async move { rt.browser.set_headful(on).await });
        cx.notify();
      })))
      .child(Button::new("im-done", t("I'm done")).size(Size::Sm).on_click(cx.listener(|this, _, _, cx| {
        if this.rt.browser.is_headful() {
          let rt = this.rt.clone();
          crate::tk::spawn(async move { rt.browser.set_headful(false).await });
        }
        this.done(cx)
      })))
  } else {
    div()
      .flex()
      .items_center()
      .gap(px(8.0))
      .child(div().flex_1().text_size(px(12.0)).text_color(ink.dimmed).child(t("Live preview of the Bot's screen.")))
      .child(Button::new("take-over", t("Take over")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, w, cx| {
        this.taking = true;
        w.focus(&this.focus, cx);
        cx.notify();
      })))
  });
  col.into_any_element()
}

#[cfg(test)]
#[path = "../../tests/screen.rs"]
mod tests;
