//! About: the version, with Copy version info.

use crate::i18n::t;
use crate::root::Root;
use gpui::prelude::*;
use gpui::{div, px, Context, WeakEntity, Window};
use guise::{Button, Modal, Size, Stack};

pub struct About {
  root: WeakEntity<Root>,
}

pub fn info() -> String {
  format!(
    "Asylum {} ({})\n{} {}",
    super::updates::VERSION,
    if cfg!(debug_assertions) { "dev" } else { "release" },
    std::env::consts::OS,
    std::env::consts::ARCH
  )
}

pub fn about(root: &mut Root, _window: &mut Window, cx: &mut Context<Root>) {
  let weak = cx.entity().downgrade();
  let view = cx.new(|_| About { root: weak });
  root.set_modal(view, cx);
}

impl Render for About {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = crate::theme::ink(cx);
    let root = self.root.clone();
    div().absolute().top_0().left_0().size_full().child(
      Modal::new()
        .width(360.0)
        .on_close(move |_, w, cx| {
          let _ = root.update(cx, |r, cx| r.close_modal(w, cx));
        })
        .child(
          Stack::new()
            .gap(Size::Sm)
            .align(guise::Align::Center)
            .child(gpui::img(std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/icon.png"))).size(px(72.0)))
            .child(div().text_size(px(18.0)).font_weight(gpui::FontWeight::SEMIBOLD).child("Asylum"))
            .child(div().text_color(ink.dimmed).child(format!("{} {}", t("Version"), super::updates::VERSION)))
            .child(Button::new("copy-version", t("Copy version info")).size(Size::Xs).on_click(|_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(info())))),
        ),
    )
  }
}
