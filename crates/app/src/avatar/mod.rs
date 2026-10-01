//! An Agent's face: an uploaded or generated image, a pixel sprite
//! ("sprite:" spec; Agents without an avatar get one picked from their id),
//! an older constructed character ("bot:"), an emoji, or its initial — with
//! a ring that animates by state: working pulses, waiting glows amber, done
//! shows a dot.

pub mod character;
pub mod picker;
pub mod sprite;

use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, Animation, AnimationExt, App, Hsla, IntoElement, ObjectFit, SharedString};
use std::time::Duration;
use store::Bot;

pub fn color_of(bot: &Bot, cx: &App) -> Hsla {
  if bot.color.starts_with('#') {
    guise::Color::hex(&bot.color).hsla()
  } else {
    ink(cx).primary
  }
}

pub fn initials(name: &str) -> String {
  let mut words = name.split_whitespace();
  let a = words.next().and_then(|w| w.chars().next());
  let b = words.next().and_then(|w| w.chars().next());
  match (a, b) {
    (Some(a), Some(b)) => format!("{}{}", a.to_uppercase(), b.to_uppercase()),
    (Some(a), None) => a.to_uppercase().to_string(),
    _ => "?".into(),
  }
}

fn is_emoji(s: &str) -> bool {
  let n = s.chars().count();
  n > 0 && n <= 4 && !s.chars().all(|c| c.is_ascii_alphanumeric())
}

/// The face alone, `size` pixels square.
pub fn face(bot: &Bot, size: f32, cx: &App) -> gpui::AnyElement {
  let bg = color_of(bot, cx);
  let base = div()
    .size(px(size))
    .rounded_full()
    .flex()
    .items_center()
    .justify_center()
    .overflow_hidden()
    .flex_none();
  if bot.avatar.starts_with('/') && std::path::Path::new(&bot.avatar).exists() {
    return base
      .child(gpui::img(std::path::PathBuf::from(&bot.avatar)).size(px(size)).object_fit(ObjectFit::Cover))
      .into_any_element();
  }
  if let Some(spec) = bot.avatar.strip_prefix("sprite:") {
    return base.bg(bg.opacity(0.42)).child(sprite::render(spec, size * 0.95)).into_any_element();
  }
  if let Some(spec) = bot.avatar.strip_prefix("bot:") {
    return base.bg(bg).child(character::render(spec, size, bg)).into_any_element();
  }
  if is_emoji(&bot.avatar) {
    return base
      .bg(bg.opacity(0.18))
      .text_size(px(size * 0.55))
      .child(SharedString::from(bot.avatar.clone()))
      .into_any_element();
  }
  if bot.avatar.trim().is_empty() && !bot.id.is_empty() {
    let spec = sprite::format(&sprite::auto(&bot.id));
    return base.bg(bg.opacity(0.42)).child(sprite::render(spec.trim_start_matches("sprite:"), size * 0.95)).into_any_element();
  }
  base
    .bg(bg)
    .text_color(gpui::white())
    .text_size(px((size * 0.4).max(9.0)))
    .font_weight(gpui::FontWeight::SEMIBOLD)
    .child(SharedString::from(initials(&bot.name)))
    .into_any_element()
}

/// The face with its state ring.
pub fn avatar(bot: &Bot, size: f32, cx: &App) -> gpui::AnyElement {
  let ink = ink(cx);
  let ring = size + 6.0;
  let holder = div().size(px(ring)).flex_none().relative().flex().items_center().justify_center();
  let inner = face(bot, size, cx);
  match bot.status.as_str() {
    "working" => holder
      .child(
        div()
          .absolute()
          .size(px(ring))
          .rounded_full()
          .border_2()
          .border_color(ink.primary)
          .with_animation(
            SharedString::from(format!("pulse-{}", bot.id)),
            Animation::new(Duration::from_millis(1400)).repeat().with_easing(gpui::pulsating_between(0.25, 1.0)),
            |el, d| el.opacity(d),
          ),
      )
      .child(inner)
      .into_any_element(),
    "waiting" => holder
      .child(div().absolute().size(px(ring)).rounded_full().border_2().border_color(ink.warning))
      .child(inner)
      .into_any_element(),
    "done" => holder
      .child(inner)
      .child(
        div()
          .absolute()
          .right_0()
          .bottom_0()
          .size(px((size * 0.32).max(7.0)))
          .rounded_full()
          .bg(ink.success)
          .border_2()
          .border_color(ink.sidebar),
      )
      .into_any_element(),
    _ => holder.child(inner).into_any_element(),
  }
}
