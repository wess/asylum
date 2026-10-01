//! The original constructed characters ("bot:" specs): simple shapes and
//! expressive eyes. New avatars are sprites; these still render for Agents
//! that have one. A spec is `shape=round;eyes=dots;accessory=antenna;tone=#hex`.

use gpui::prelude::*;
use gpui::{div, px, Hsla, IntoElement};

pub const TONES: [&str; 8] = ["#7c5cff", "#ff6b6b", "#1fb6ff", "#13ce66", "#ffb020", "#ff49db", "#00c2a8", "#8e44ad"];

#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
  pub shape: String,
  pub eyes: String,
  pub accessory: String,
  pub tone: String,
}

impl Default for Spec {
  fn default() -> Self {
    Self { shape: "round".into(), eyes: "dots".into(), accessory: "antenna".into(), tone: TONES[0].into() }
  }
}

pub fn parse(spec: &str) -> Spec {
  let mut s = Spec::default();
  for kv in spec.split(';') {
    if let Some((k, v)) = kv.split_once('=') {
      match k.trim() {
        "shape" => s.shape = v.trim().into(),
        "eyes" => s.eyes = v.trim().into(),
        "accessory" => s.accessory = v.trim().into(),
        "tone" => s.tone = v.trim().into(),
        _ => {}
      }
    }
  }
  s
}

pub fn render(spec: &str, size: f32, _bg: Hsla) -> impl IntoElement {
  let s = parse(spec);
  let tone = guise::Color::hex(&s.tone).hsla();
  let (w, h, r) = match s.shape.as_str() {
    "square" => (0.62, 0.56, 0.12),
    "tall" => (0.5, 0.64, 0.22),
    "wide" => (0.7, 0.48, 0.2),
    _ => (0.6, 0.56, 0.3),
  };
  let head = div()
    .w(px(size * w))
    .h(px(size * h))
    .rounded(px(size * r))
    .bg(gpui::white())
    .flex()
    .items_center()
    .justify_center()
    .gap(px(size * 0.1));
  let eye = |shape: &str| {
    let e = div().bg(tone);
    match shape {
      "ovals" => e.w(px(size * 0.08)).h(px(size * 0.14)).rounded_full(),
      "happy" => e.w(px(size * 0.12)).h(px(size * 0.05)).rounded_t(px(size * 0.06)),
      "visor" => e.w(px(size * 0.4)).h(px(size * 0.08)).rounded(px(size * 0.04)),
      _ => e.size(px(size * 0.09)).rounded_full(),
    }
  };
  let eyes = match s.eyes.as_str() {
    "visor" => head.child(eye("visor")),
    "wink" => head.child(eye("dots")).child(eye("happy")),
    e => head.child(eye(e)).child(eye(e)),
  };
  let mut col = div().flex().flex_col().items_center();
  col = match s.accessory.as_str() {
    "antenna" => col.child(div().size(px(size * 0.1)).rounded_full().bg(gpui::white())).child(div().w(px(size * 0.03)).h(px(size * 0.06)).bg(gpui::white())),
    "halo" => col.child(div().w(px(size * 0.36)).h(px(size * 0.06)).rounded_full().border_2().border_color(gpui::yellow())),
    "bow" => col.child(div().w(px(size * 0.22)).h(px(size * 0.08)).rounded(px(size * 0.03)).bg(gpui::red())),
    "headset" => col.child(div().w(px(size * 0.66)).h(px(size * 0.05)).rounded_t(px(size * 0.2)).border_t_2().border_color(gpui::white())),
    _ => col,
  };
  col.child(eyes)
}

#[cfg(test)]
#[path = "../../tests/character.rs"]
mod tests;
