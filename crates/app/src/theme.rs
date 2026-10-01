//! The guise theme: light or dark (following the system unless the user
//! picked one), with the brand violet as primary.

use config::Appearance;
use gpui::{App, Hsla, WindowAppearance};
use guise::Theme;

pub const BRAND: &str = "#7c5cff";

pub fn dark_for(choice: Appearance, system: WindowAppearance) -> bool {
  match choice {
    Appearance::Light => false,
    Appearance::Dark => true,
    Appearance::System => matches!(system, WindowAppearance::Dark | WindowAppearance::VibrantDark),
  }
}

pub fn install(dark: bool, cx: &mut App) {
  let brand: Hsla = guise::Color::hex(BRAND).hsla();
  let t = if dark {
    Theme::dark()
      .with_primary(brand)
      .with_body(guise::Color::hex("#131318").hsla())
      .with_surface(guise::Color::hex("#1b1b22").hsla())
      .with_surface_hover(guise::Color::hex("#24242d").hsla())
      .with_border(guise::Color::hex("#2c2c36").hsla())
  } else {
    Theme::light()
      .with_primary(brand)
      .with_body(guise::Color::hex("#ffffff").hsla())
      .with_surface(guise::Color::hex("#f6f6f8").hsla())
      .with_surface_hover(guise::Color::hex("#ededf1").hsla())
      .with_border(guise::Color::hex("#e4e4ea").hsla())
  };
  let mut t = t;
  t.primary_color = guise::ColorName::Violet;
  // guise components default to Blue; make that family the brand violet.
  let violet = t.palette.shades(guise::ColorName::Violet);
  t.palette.set_shades(guise::ColorName::Blue, violet);
  t.init(cx);
}

/// Theme colors as gpui colors.
pub struct Ink {
  pub body: Hsla,
  pub surface: Hsla,
  pub hover: Hsla,
  pub border: Hsla,
  pub text: Hsla,
  pub dimmed: Hsla,
  pub primary: Hsla,
  pub danger: Hsla,
  pub success: Hsla,
  pub warning: Hsla,
  pub sidebar: Hsla,
}

pub fn ink(cx: &App) -> Ink {
  let t = guise::theme(cx);
  let dark = t.scheme.is_dark();
  Ink {
    body: t.body().hsla(),
    surface: t.surface().hsla(),
    hover: t.surface_hover().hsla(),
    border: t.border().hsla(),
    text: t.text().hsla(),
    dimmed: t.dimmed().hsla(),
    primary: t.primary().hsla(),
    danger: t.danger().hsla(),
    success: t.success().hsla(),
    warning: t.warning().hsla(),
    sidebar: if dark { guise::Color::hex("#0f0f13").hsla() } else { guise::Color::hex("#f4f3f8").hsla() },
  }
}
