//! The avatar picker: build a character (shapes, eyes, accessories,
//! colors), generate one from a prompt, or upload an image (under 25 MB).

use super::character::{self, Spec};
use crate::i18n::t;
use crate::root::Root;
use crate::theme::ink;
use crate::tk;
use agent::Runtime;
use gpui::prelude::*;
use gpui::{div, px, Context, Entity, ExternalPaths, PathPromptOptions, SharedString, WeakEntity, Window};
use guise::{Button, Group, Modal, SegmentedControl, SegmentedControlEvent, Size, TextInput, Variant};
use std::path::PathBuf;

pub const MAX_UPLOAD: u64 = 25 * 1024 * 1024;

pub struct Picker {
  rt: Runtime,
  root: WeakEntity<Root>,
  bot: store::Bot,
  tabs: Entity<SegmentedControl>,
  tab: usize,
  spec: Spec,
  prompt: Entity<TextInput>,
  preview: Option<String>,
  busy: bool,
  error: Option<String>,
  _sub: gpui::Subscription,
}

pub fn open(root: &mut Root, bot: &str, window: &mut Window, cx: &mut Context<Root>) {
  let Some(b) = root.snap.bot(bot).cloned() else { return };
  let rt = root.rt.clone();
  let weak = cx.entity().downgrade();
  let view = cx.new(|cx| {
    let tabs = cx.new(|cx| SegmentedControl::new(cx).data([t("Bot"), t("Generate"), t("Upload")]).selected(0).size(Size::Sm));
    let sub = cx.subscribe(&tabs, |this: &mut Picker, _, ev: &SegmentedControlEvent, cx| {
      this.tab = ev.0;
      cx.notify();
    });
    let spec = b.avatar.strip_prefix("bot:").map(character::parse).unwrap_or_default();
    let prompt = cx.new(|cx| TextInput::new(cx).placeholder(t("A friendly robot with round glasses, flat illustration")));
    let _ = window;
    Picker { rt, root: weak, bot: b, tabs, tab: 0, spec, prompt, preview: None, busy: false, error: None, _sub: sub }
  });
  root.set_modal(view, cx);
}

impl Picker {
  fn set(&mut self, avatar: String, window: &mut Window, cx: &mut Context<Self>) {
    let mut p = self.bot.profile();
    p.avatar = avatar;
    let rt = self.rt.clone();
    let id = self.bot.id.clone();
    tk::spawn(async move { agent::api::bots::update(&rt, &id, &p).await });
    let _ = self.root.update(cx, |r, cx| r.close_modal(window, cx));
  }

  fn upload(&mut self, path: PathBuf, cx: &mut Context<Self>) {
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if size == 0 || size > MAX_UPLOAD {
      self.error = Some(t("Images must be under 25 MB.").into());
      cx.notify();
      return;
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if !["png", "jpg", "jpeg", "gif", "webp", "heic"].contains(&ext.as_str()) {
      self.error = Some(t("Pick a PNG, JPEG, GIF, or WebP image.").into());
      cx.notify();
      return;
    }
    let dir = config::data_dir().join("avatars");
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(format!("{}-{}.{ext}", self.bot.id, store::now()));
    // Square-crop and shrink with sips so avatars stay small.
    let _ = std::fs::copy(&path, &dest);
    let _ = std::process::Command::new("sips").args(["--cropToHeightWidth", "512", "512", "-Z", "512"]).arg(&dest).output();
    self.preview = Some(dest.display().to_string());
    self.error = None;
    cx.notify();
  }

  fn generate(&mut self, cx: &mut Context<Self>) {
    let prompt = self.prompt.read(cx).text();
    if prompt.trim().is_empty() {
      return;
    }
    self.busy = true;
    self.error = None;
    let rt = self.rt.clone();
    let id = self.bot.id.clone();
    cx.spawn(async move |this, cx| {
      let r = tk::run(async move {
        let bytes = agent::media::image(&rt, &format!("Square avatar, centered subject, simple background: {prompt}")).await?;
        let dir = config::data_dir().join("avatars");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{id}-{}.png", store::now()));
        std::fs::write(&path, bytes)?;
        Ok(path.display().to_string())
      })
      .await;
      let _ = this.update(cx, |this, cx| {
        this.busy = false;
        match r {
          Ok(p) => this.preview = Some(p),
          Err(e) => this.error = Some(e.to_string()),
        }
        cx.notify();
      });
    })
    .detach();
  }
}

fn options(label: &'static str, values: &'static [&'static str], current: &str, cx: &mut Context<Picker>, set: fn(&mut Spec, &str)) -> impl IntoElement {
  let ink = ink(cx);
  let mut row = div().flex().flex_wrap().gap(px(4.0)).items_center().child(div().w(px(84.0)).text_size(px(12.0)).text_color(ink.dimmed).child(t(label)));
  for v in values {
    let on = *v == current;
    row = row.child(
      crate::details::profile::chip(SharedString::from(format!("{label}-{v}")), if label == "Color" { "●".into() } else { t(v).into() }, on, &ink)
        .when(label == "Color", |d| d.text_color(guise::Color::hex(v).hsla()))
        .on_click(cx.listener(move |this, _, _, cx| {
          set(&mut this.spec, v);
          cx.notify();
        })),
    );
  }
  row
}

impl Render for Picker {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let root = self.root.clone();
    let mut preview_bot = self.bot.clone();
    let mut body = div().flex().flex_col().gap(px(10.0)).child(self.tabs.clone());
    match self.tab {
      0 => {
        preview_bot.avatar = character::format(&self.spec);
        let spec = self.spec.clone();
        body = body
          .child(div().flex().justify_center().py(px(8.0)).child(super::face(&preview_bot, 96.0, cx)))
          .child(options("Shape", &character::SHAPES, &spec.shape, cx, |s, v| s.shape = v.into()))
          .child(options("Eyes", &character::EYES, &spec.eyes, cx, |s, v| s.eyes = v.into()))
          .child(options("Accessory", &character::ACCESSORIES, &spec.accessory, cx, |s, v| s.accessory = v.into()))
          .child(options("Color", &character::TONES, &spec.tone, cx, |s, v| s.tone = v.into()))
          .child(Button::new("set-char", t("Set avatar")).on_click(cx.listener(|this, _, w, cx| {
            let a = character::format(&this.spec);
            this.set(a, w, cx);
          })));
      }
      1 => {
        body = body.child(self.prompt.clone()).child(
          Group::new()
            .gap(Size::Sm)
            .child(Button::new("gen", if self.busy { t("Generating…") } else { t("Generate") }).disabled(self.busy).on_click(cx.listener(|this, _, _, cx| this.generate(cx))))
            .when(self.preview.is_some(), |g| g.child(Button::new("set-gen", t("Set avatar")).variant(Variant::Light).on_click(cx.listener(|this, _, w, cx| {
              if let Some(p) = this.preview.clone() {
                this.set(p, w, cx);
              }
            })))),
        );
      }
      _ => {
        body = body
          .child(
            div()
              .id("drop")
              .h(px(120.0))
              .flex()
              .flex_col()
              .items_center()
              .justify_center()
              .gap(px(6.0))
              .rounded(px(10.0))
              .border_2()
              .border_dashed()
              .border_color(ink.border)
              .drag_over::<ExternalPaths>(move |s, _, _, _| s.border_color(ink.primary))
              .on_drop(cx.listener(|this, p: &ExternalPaths, _, cx| {
                if let Some(first) = p.paths().first() {
                  this.upload(first.clone(), cx);
                }
              }))
              .child(div().text_color(ink.dimmed).child(t("Drag an image here")))
              .child(Button::new("browse", t("Browse files")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|_, _, _, cx| {
                let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: None });
                cx.spawn(async move |this, cx| {
                  if let Ok(Ok(Some(paths))) = rx.await {
                    if let Some(p) = paths.into_iter().next() {
                      let _ = this.update(cx, |this, cx| this.upload(p, cx));
                    }
                  }
                })
                .detach();
              }))),
          )
          .when(self.preview.is_some(), |b| b.child(Button::new("set-up", t("Set avatar")).on_click(cx.listener(|this, _, w, cx| {
            if let Some(p) = this.preview.clone() {
              this.set(p, w, cx);
            }
          }))));
      }
    }
    if let Some(p) = &self.preview {
      if self.tab > 0 {
        preview_bot.avatar = p.clone();
        body = body.child(div().flex().justify_center().child(super::face(&preview_bot, 96.0, cx)));
      }
    }
    if let Some(e) = &self.error {
      body = body.child(div().text_size(px(12.0)).text_color(ink.danger).child(SharedString::from(e.clone())));
    }
    div().absolute().top_0().left_0().size_full().child(
      Modal::new()
        .title(t("Avatar"))
        .width(460.0)
        .on_close(move |_, w, cx| {
          let _ = root.update(cx, |r, cx| r.close_modal(w, cx));
        })
        .child(body),
    )
  }
}
