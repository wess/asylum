//! The shared workspace as a file manager: browse folders, open, reveal,
//! delete, and drop files in.

use super::Panel;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, ExternalPaths, SharedString};
use guise::{ActionIcon, IconName, Size, Variant};

impl Panel {
  pub fn list(&mut self, cx: &mut Context<Self>) {
    let ws = self.rt.computer.workspace();
    self.entries = computer::fs::list(&ws, &self.dir).unwrap_or_default();
    cx.notify();
  }
}

fn size(n: u64) -> String {
  match n {
    n if n >= 1 << 20 => format!("{:.1} MB", n as f64 / 1048576.0),
    n if n >= 1024 => format!("{} KB", n / 1024),
    n => format!("{n} B"),
  }
}

pub fn render(p: &mut Panel, cx: &mut Context<Panel>) -> AnyElement {
  let ink = ink(cx);
  let ws = p.rt.computer.workspace();
  let mut list = div().id("files").flex().flex_col().flex_1().overflow_y_scroll();
  if p.dir != "~" {
    list = list.child(
      div()
        .id("up")
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(8.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(ink.hover))
        .on_click(cx.listener(|this, _, _, cx| {
          let up = this.dir.rsplit_once('/').map(|(a, _)| a.to_string()).unwrap_or_else(|| "~".into());
          this.dir = if up.is_empty() { "~".into() } else { up };
          this.list(cx);
        }))
        .child(guise::Icon::new(IconName::CornerLeftUp).size(Size::Xs))
        .child(".."),
    );
  }
  for e in p.entries.clone() {
    let (path, path2, path3) = (e.path.clone(), e.path.clone(), e.path.clone());
    let full = ws.join(e.path.trim_start_matches("~/"));
    let full2 = full.clone();
    list = list.child(
      div()
        .id(SharedString::from(format!("f-{}", e.path)))
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(8.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(ink.hover))
        .on_click(cx.listener(move |this, _, _, cx| {
          if full.is_dir() {
            this.dir = path.clone();
            this.list(cx);
          } else {
            cx.open_url(&format!("file://{}", full.display()));
          }
        }))
        .child(div().text_color(if e.dir { ink.primary } else { ink.dimmed }).child(guise::Icon::new(if e.dir { IconName::Folder } else { IconName::File }).size(Size::Sm)))
        .child(div().flex_1().truncate().text_size(px(13.0)).child(e.name.clone()))
        .when(!e.dir, |d| d.child(div().text_size(px(11.0)).text_color(ink.dimmed).child(size(e.size))))
        .child(ActionIcon::new(SharedString::from(format!("rv-{path2}")), IconName::FolderOpen).size(Size::Xs).variant(Variant::Subtle).on_click(move |_, _, cx| cx.reveal_path(&full2)))
        .child(ActionIcon::new(SharedString::from(format!("rmf-{path3}")), IconName::Trash2).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(move |this, _, _, cx| {
          let ws = this.rt.computer.workspace();
          if let Err(e) = computer::fs::remove(&ws, &path3) {
            this.status = Some(e.to_string());
          }
          this.list(cx);
        }))),
    );
  }
  if p.entries.is_empty() {
    list = list.child(div().p(px(12.0)).text_size(px(13.0)).text_color(ink.dimmed).child(t("This folder is empty. Drop files here to add them.")));
  }
  div()
    .id("files-drop")
    .flex()
    .flex_col()
    .flex_1()
    .min_h_0()
    .gap(px(6.0))
    .drag_over::<ExternalPaths>(move |s, _, _, _| s.bg(ink.primary.opacity(0.08)))
    .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
      let ws = this.rt.computer.workspace();
      for src in paths.paths() {
        if let Some(name) = src.file_name() {
          let rel = format!("{}/{}", this.dir, name.to_string_lossy());
          if let Ok(dest) = computer::fs::resolve(&ws, &rel) {
            if src.is_dir() {
              let _ = computer::copy_dir(src, &dest);
            } else {
              let _ = std::fs::copy(src, dest);
            }
          }
        }
      }
      this.list(cx);
    }))
    .child(div().flex().items_center().gap(px(6.0)).text_size(px(12.0)).text_color(ink.dimmed).child(guise::Icon::new(IconName::HardDrive).size(Size::Xs)).child(SharedString::from(format!("workspace/{}", p.dir.trim_start_matches('~').trim_start_matches('/')))))
    .child(list)
    .into_any_element()
}
