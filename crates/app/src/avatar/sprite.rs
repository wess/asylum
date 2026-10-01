//! Pixel sprites in the 8/16-bit RPG tradition: a 16×16 grid per class,
//! painted from four user colors. A spec is
//! `sprite:<class>;primary=#hex;accent=#hex;hair=#hex;skin=#hex`; colors left
//! out fall back to the class's own.
//!
//! Grid letters: `.` clear, `o` outline, `e` eyes, `s`/`S` skin and its
//! shadow, `h`/`H` hair, `p`/`P` primary (clothes), `a`/`A` accent (trim,
//! hats, hoods), `m`/`M` metal, `w` white, `b`/`B` wood and leather, `g` gold.

use gpui::prelude::*;
use gpui::{div, px, rgb, IntoElement};

pub const SIZE: usize = 16;

pub struct Class {
  pub id: &'static str,
  pub label: &'static str,
  pub grid: &'static str,
}

pub const CLASSES: [Class; 8] = [
  Class { id: "warrior", label: "Warrior", grid: include_str!("../../sprites/warrior.txt") },
  Class { id: "knight", label: "Knight", grid: include_str!("../../sprites/knight.txt") },
  Class { id: "wizard", label: "Wizard", grid: include_str!("../../sprites/wizard.txt") },
  Class { id: "healer", label: "Healer", grid: include_str!("../../sprites/healer.txt") },
  Class { id: "thief", label: "Thief", grid: include_str!("../../sprites/thief.txt") },
  Class { id: "monk", label: "Monk", grid: include_str!("../../sprites/monk.txt") },
  Class { id: "ranger", label: "Ranger", grid: include_str!("../../sprites/ranger.txt") },
  Class { id: "sage", label: "Sage", grid: include_str!("../../sprites/sage.txt") },
];

/// Swatches offered for clothes and trim.
pub const CLOTH: [&str; 12] = [
  "#b83b3b", "#e07b2c", "#d4a017", "#2f8f5b", "#2a9d8f", "#2f5fb3", "#3b3f8f", "#6b3fa0", "#c2185b", "#8b5a2b", "#c9cfd8", "#f2efe8",
];
pub const HAIR: [&str; 8] = ["#e6a23c", "#c46a2f", "#6b4a2b", "#4a3426", "#2b2b2b", "#d8d8d8", "#3d6fd1", "#c2185b"];
pub const SKIN: [&str; 6] = ["#f6d2b5", "#f2c7a0", "#e8b48c", "#d9a06f", "#a8704a", "#7a4e33"];

const OUTLINE: u32 = 0x1b1426;

#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
  pub class: String,
  pub primary: String,
  pub accent: String,
  pub hair: String,
  pub skin: String,
}

fn class(id: &str) -> &'static Class {
  CLASSES.iter().find(|c| c.id == id).unwrap_or(&CLASSES[0])
}

/// The class's own colors from its `# key=#hex …` header line.
fn defaults(c: &Class) -> Spec {
  let mut s = Spec { class: c.id.into(), primary: String::new(), accent: String::new(), hair: String::new(), skin: String::new() };
  for line in c.grid.lines().filter(|l| l.starts_with('#')) {
    for kv in line.trim_start_matches('#').split_whitespace() {
      if let Some((k, v)) = kv.split_once('=') {
        set(&mut s, k, v);
      }
    }
  }
  s
}

fn set(s: &mut Spec, k: &str, v: &str) {
  if hex(v).is_none() {
    return;
  }
  match k {
    "primary" => s.primary = v.into(),
    "accent" => s.accent = v.into(),
    "hair" => s.hair = v.into(),
    "skin" => s.skin = v.into(),
    _ => {}
  }
}

pub fn of_class(id: &str) -> Spec {
  defaults(class(id))
}

/// A stable sprite for an Agent that never picked one, chosen from a seed
/// (its id) so it looks the same everywhere and across launches.
pub fn auto(seed: &str) -> Spec {
  // FNV-1a: stable across Rust versions, unlike `DefaultHasher`.
  let h = seed.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
  let pick = |list: &[&str], shift: u32| list[((h >> shift) as usize) % list.len()].to_string();
  let mut s = of_class(CLASSES[(h % CLASSES.len() as u64) as usize].id);
  s.primary = pick(&CLOTH, 8);
  s.hair = pick(&HAIR, 16);
  s.skin = pick(&SKIN, 24);
  s
}

pub fn parse(spec: &str) -> Spec {
  let mut parts = spec.split(';');
  let mut s = of_class(parts.next().unwrap_or("").trim());
  for kv in parts {
    if let Some((k, v)) = kv.split_once('=') {
      set(&mut s, k.trim(), v.trim());
    }
  }
  s
}

pub fn format(s: &Spec) -> String {
  format!("sprite:{};primary={};accent={};hair={};skin={}", s.class, s.primary, s.accent, s.hair, s.skin)
}

pub fn hex(v: &str) -> Option<u32> {
  let h = v.strip_prefix('#')?;
  if h.len() != 6 {
    return None;
  }
  u32::from_str_radix(h, 16).ok()
}

fn shade(c: u32, f: f32) -> u32 {
  let ch = |shift: u32| ((((c >> shift) & 0xff) as f32 * f) as u32).min(255) << shift;
  ch(16) | ch(8) | ch(0)
}

/// Each pixel's color, row by row; `None` is transparent.
pub fn pixels(s: &Spec) -> Vec<Vec<Option<u32>>> {
  let base = defaults(class(&s.class));
  let pick = |v: &str, d: &str| hex(v).or_else(|| hex(d)).unwrap_or(0x808080);
  let p = pick(&s.primary, &base.primary);
  let a = pick(&s.accent, &base.accent);
  let h = pick(&s.hair, &base.hair);
  let k = pick(&s.skin, &base.skin);
  let color = |ch: char| match ch {
    'o' | 'e' => Some(OUTLINE),
    's' => Some(k),
    'S' => Some(shade(k, 0.82)),
    'h' => Some(h),
    'H' => Some(shade(h, 0.72)),
    'p' => Some(p),
    'P' => Some(shade(p, 0.72)),
    'a' => Some(a),
    'A' => Some(shade(a, 0.72)),
    'm' => Some(0xc9d1dc),
    'M' => Some(0x7d8798),
    'w' => Some(0xffffff),
    'b' => Some(0x7a4a2a),
    'B' => Some(0x4e2e1a),
    'g' => Some(0xffd866),
    _ => None,
  };
  class(&s.class)
    .grid
    .lines()
    .filter(|l| !l.is_empty() && !l.starts_with('#'))
    .take(SIZE)
    .map(|l| l.chars().take(SIZE).map(color).collect())
    .collect()
}

/// Runs of one color in a row, so a sprite is a few dozen boxes instead of
/// 256 and adjacent pixels never show hairline seams.
pub fn runs(row: &[Option<u32>]) -> Vec<(usize, usize, Option<u32>)> {
  let mut out: Vec<(usize, usize, Option<u32>)> = Vec::new();
  for (x, c) in row.iter().enumerate() {
    match out.last_mut() {
      Some((_, n, last)) if last == c => *n += 1,
      _ => out.push((x, 1, *c)),
    }
  }
  out
}

/// Rows and columns shown at `size`: the whole figure when there's room for
/// at least 3 screen pixels per sprite pixel, else a head-and-shoulders
/// portrait (rows 1–12, columns 2–13), like a menu portrait.
pub fn frame(size: f32) -> (std::ops::Range<usize>, std::ops::Range<usize>) {
  if size >= 3.0 * SIZE as f32 {
    (0..SIZE, 0..SIZE)
  } else {
    (1..13, 2..14)
  }
}

/// The sprite drawn at most `size` points square, at a whole-point scale so
/// edges stay crisp on every display.
pub fn render(spec: &str, size: f32) -> impl IntoElement {
  let s = parse(spec);
  let (rows, cols) = frame(size);
  let px1 = (size / cols.len() as f32).floor().max(1.0);
  let mut col = div().flex().flex_col().w(px(px1 * cols.len() as f32)).flex_none();
  for row in pixels(&s).into_iter().skip(rows.start).take(rows.len()) {
    let mut line = div().flex().h(px(px1)).flex_none();
    for (_, n, c) in runs(&row[cols.clone()]) {
      let cell = div().w(px(px1 * n as f32)).h_full().flex_none();
      line = line.child(match c {
        Some(c) => cell.bg(rgb(c)),
        None => cell,
      });
    }
    col = col.child(line);
  }
  col
}

#[cfg(test)]
#[path = "../../tests/sprite.rs"]
mod tests;
