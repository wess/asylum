//! Files, images, links, voice memos, and voice chat records.

use super::shell;
use crate::chat::ChatPane;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, ObjectFit, SharedString};
use guise::{ActionIcon, IconName, Size, Variant};
use std::path::PathBuf;
use store::Message;

fn human(n: u64) -> String {
  match n {
    n if n >= 1 << 20 => format!("{:.1} MB", n as f64 / (1 << 20) as f64),
    n if n >= 1 << 10 => format!("{:.0} KB", n as f64 / 1024.0),
    n => format!("{n} B"),
  }
}

fn icon_for(mime: &str) -> IconName {
  if mime.starts_with("image/") {
    IconName::Image
  } else if mime.starts_with("audio/") {
    IconName::FileAudio
  } else if mime.starts_with("video/") {
    IconName::FileVideo
  } else if mime.contains("csv") || mime.contains("sheet") {
    IconName::FileSpreadsheet
  } else if mime.contains("json") || mime.contains("yaml") {
    IconName::FileCode
  } else {
    IconName::FileText
  }
}

pub fn file(pane: &mut ChatPane, name: &str, path: &str, mime: &str, size: Option<u64>, cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  if mime.starts_with("image/") {
    return image(pane, path, name, cx);
  }
  let (p1, p2, p3) = (path.to_string(), path.to_string(), path.to_string());
  let meta = size.map(human).unwrap_or_else(|| mime.to_string());
  div()
    .id(SharedString::from(format!("file-{path}")))
    .flex()
    .items_center()
    .gap(px(10.0))
    .p(px(10.0))
    .rounded(px(10.0))
    .border_1()
    .border_color(ink.border)
    .bg(ink.surface)
    .max_w(px(420.0))
    .cursor_pointer()
    .on_click(move |_, _, cx| cx.open_url(&format!("file://{p1}")))
    .child(div().text_color(ink.primary).child(guise::Icon::new(icon_for(mime)).size(Size::Md)))
    .child(
      div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(div().truncate().text_size(px(13.0)).child(SharedString::from(name.to_string())))
        .child(div().text_size(px(11.0)).text_color(ink.dimmed).child(meta)),
    )
    .child(
      ActionIcon::new(SharedString::from(format!("reveal-{p2}")), IconName::FolderOpen)
        .size(Size::Xs)
        .variant(Variant::Subtle)
        .on_click(move |_, _, cx| cx.reveal_path(&PathBuf::from(&p2))),
    )
    .child(
      ActionIcon::new(SharedString::from(format!("save-{p3}")), IconName::Download)
        .size(Size::Xs)
        .variant(Variant::Subtle)
        .on_click(move |_, _, cx| save_as(&p3, cx)),
    )
    .into_any_element()
}

/// Save a copy wherever the user picks.
pub fn save_as(path: &str, cx: &mut gpui::App) {
  let src = PathBuf::from(path);
  let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
  let dir = std::env::var("HOME").map(|h| PathBuf::from(h).join("Downloads")).unwrap_or_default();
  let rx = cx.prompt_for_new_path(&dir, Some(&name));
  cx.spawn(async move |_| {
    if let Ok(Ok(Some(dest))) = rx.await {
      let _ = std::fs::copy(&src, dest);
    }
  })
  .detach();
}

pub fn image(pane: &mut ChatPane, path: &str, caption: &str, cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  let _ = pane;
  let (p1, p2) = (path.to_string(), path.to_string());
  div()
    .id(SharedString::from(format!("img-{path}")))
    .flex()
    .flex_col()
    .gap(px(4.0))
    .max_w(px(460.0))
    .child(
      div()
        .rounded(px(10.0))
        .overflow_hidden()
        .border_1()
        .border_color(ink.border)
        .cursor_pointer()
        .id(SharedString::from(format!("imgc-{path}")))
        .on_click(move |_, _, cx| cx.open_url(&format!("file://{p1}")))
        .child(gpui::img(PathBuf::from(path)).max_w(px(460.0)).max_h(px(360.0)).object_fit(ObjectFit::Contain)),
    )
    .child(
      div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .text_size(px(11.0))
        .text_color(ink.dimmed)
        .child(div().flex_1().truncate().child(SharedString::from(caption.to_string())))
        .child(ActionIcon::new(SharedString::from(format!("imgsave-{p2}")), IconName::Download).size(Size::Xs).variant(Variant::Subtle).on_click(move |_, _, cx| save_as(&p2, cx))),
    )
    .into_any_element()
}

pub fn link(pane: &mut ChatPane, url: &str, title: &str, cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  let host = url.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or(url).to_string();
  let u = url.to_string();
  let preview = pane.previews.get(url).cloned().flatten();
  let tip = match &preview {
    Some(p) if !p.description.is_empty() => format!("{}\n\n{}\n\n{url}", if p.title.is_empty() { &host } else { &p.title }, p.description),
    _ => url.to_string(),
  };
  let hover_url = url.to_string();
  div()
    .id(SharedString::from(format!("link-{url}")))
    .flex()
    .items_center()
    .gap(px(10.0))
    .p(px(10.0))
    .rounded(px(10.0))
    .border_1()
    .border_color(ink.border)
    .max_w(px(460.0))
    .cursor_pointer()
    .hover(|s| s.bg(ink.hover))
    .tooltip(guise::tooltip(tip))
    .on_hover(cx.listener(move |this, on: &bool, _, cx| {
      // Fetch the preview the first time the card is hovered.
      if !*on || this.previews.contains_key(&hover_url) {
        return;
      }
      this.previews.insert(hover_url.clone(), None);
      let (rt, url) = (this.rt.clone(), hover_url.clone());
      cx.spawn(async move |this, cx| {
        let r = crate::tk::run({
          let url = url.clone();
          async move { agent::api::links::preview(&rt, &url).await }
        })
        .await;
        let _ = this.update(cx, |this, cx| {
          this.previews.insert(url, r.ok());
          cx.notify();
        });
      })
      .detach();
    }))
    .on_click(move |_, _, cx| cx.open_url(&u))
    .child(div().text_color(ink.dimmed).child(guise::Icon::new(IconName::Globe).size(Size::Sm)))
    .child(
      div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(div().truncate().text_size(px(13.0)).child(SharedString::from(match (&preview, title.is_empty()) {
          (_, false) => title.to_string(),
          (Some(p), true) if !p.title.is_empty() => p.title.clone(),
          _ => host.clone(),
        })))
        .when_some(preview.as_ref().filter(|p| !p.description.is_empty()), |d, p| d.child(div().text_size(px(12.0)).text_color(ink.dimmed).line_clamp(2).child(SharedString::from(p.description.clone()))))
        .child(div().truncate().text_size(px(11.0)).text_color(ink.dimmed).child(SharedString::from(if let Some(site) = preview.as_ref().map(|p| p.site.clone()).filter(|s| !s.is_empty()) { format!("{site} · {host}") } else { host }))),
    )
    .child(div().text_color(ink.dimmed).child(guise::Icon::new(IconName::ExternalLink).size(Size::Xs)))
    .into_any_element()
}

pub fn memo(pane: &mut ChatPane, m: &Message, path: &str, transcript: &str, cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  let key = format!("{}:memo", m.id);
  let open = pane.open.contains(&key);
  let playing = crate::audio::playing(path);
  let p = path.to_string();
  let k2 = key.clone();
  let mut card = shell(cx).max_w(px(420.0)).child(
    div()
      .flex()
      .items_center()
      .gap(px(10.0))
      .child(
        div().id(SharedString::from(format!("playtip-{}", m.id))).tooltip(guise::tooltip(if playing { t("Pause voice memo") } else { t("Play voice memo") })).child(
          ActionIcon::new(SharedString::from(format!("play-{}", m.id)), if playing { IconName::Pause } else { IconName::Play })
            .variant(Variant::Filled)
            .on_click(cx.listener(move |_, _, _, cx| {
              crate::audio::toggle(&p);
              cx.notify();
            })),
        ),
      )
      .child(div().flex_1().text_size(px(13.0)).child(t("Voice memo")))
      .child(
        div()
          .id(SharedString::from(format!("tr-{}", m.id)))
          .text_size(px(12.0))
          .text_color(ink.primary)
          .cursor_pointer()
          .child(if open { t("Hide transcript") } else { t("Show transcript") })
          .on_click(cx.listener(move |this, _, _, cx| {
            if !this.open.remove(&k2) {
              this.open.insert(k2.clone());
            }
            cx.notify();
          })),
      ),
  );
  if open {
    card = card.child(div().text_size(px(13.0)).text_color(ink.dimmed).child(SharedString::from(transcript.to_string())));
  }
  card.into_any_element()
}

pub fn voicechat(pane: &mut ChatPane, m: &Message, seconds: u64, transcript: &str, cx: &mut Context<ChatPane>) -> AnyElement {
  let ink = ink(cx);
  let key = format!("{}:call", m.id);
  let open = pane.open.contains(&key);
  let k2 = key.clone();
  let dur = format!("{}:{:02}", seconds / 60, seconds % 60);
  let mut card = shell(cx).child(
    div()
      .flex()
      .items_center()
      .gap(px(8.0))
      .child(div().text_color(ink.primary).child(guise::Icon::new(IconName::AudioLines).size(Size::Sm)))
      .child(div().font_weight(gpui::FontWeight::MEDIUM).child(t("Voice chat")))
      .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(dur))
      .child(div().flex_1())
      .child(
        div()
          .id(SharedString::from(format!("calltr-{}", m.id)))
          .text_size(px(12.0))
          .text_color(ink.primary)
          .cursor_pointer()
          .child(if open { t("Hide transcript") } else { t("Show transcript") })
          .on_click(cx.listener(move |this, _, _, cx| {
            if !this.open.remove(&k2) {
              this.open.insert(k2.clone());
            }
            cx.notify();
          })),
      ),
  );
  if open {
    card = card.child(guise::Markdown::new(transcript.to_string()).size(Size::Sm));
  }
  card.into_any_element()
}
