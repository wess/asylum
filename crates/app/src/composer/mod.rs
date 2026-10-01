//! The composer: type (Enter sends, Shift+Enter adds a line), attach up to
//! six files (button, drag and drop), mention with @, reference skills with
//! /, dictate, or start a voice chat when it's empty.

pub mod dictate;
pub mod picker;

use crate::chat::ChatPane;
use crate::i18n::t;
use crate::theme::ink;
use crate::tk;
use agent::Runtime;
use gpui::prelude::*;
use gpui::{div, px, Context, Entity, ExternalPaths, PathPromptOptions, SharedString, Subscription, WeakEntity, Window};
use guise::{ActionIcon, IconName, Size, TextArea, TextAreaEvent, TextAreaSubmit, Variant};
use std::path::PathBuf;

pub struct Composer {
  pub rt: Runtime,
  pub chat: String,
  pub thread: Option<String>,
  pub pane: WeakEntity<ChatPane>,
  pub input: Entity<TextArea>,
  pub files: Vec<PathBuf>,
  pub picker: Option<picker::Picker>,
  pub editing: Option<String>,
  pub dictation: Option<dictate::Dictation>,
  pub _subs: Vec<Subscription>,
}

impl Composer {
  pub fn new(rt: Runtime, chat: String, thread: Option<String>, pane: WeakEntity<ChatPane>, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let enter = rt.settings().send_on_enter;
    let placeholder = if thread.is_some() { t("Reply in thread…") } else { t("Message") };
    let input = cx.new(|cx| TextArea::new(cx).placeholder(placeholder).rows(1).max_rows(12).submit_on_enter(enter));
    let s1 = cx.subscribe_in(&input, window, |this: &mut Composer, _, ev: &TextAreaSubmit, w, cx| {
      let _ = ev;
      this.submit(w, cx);
    });
    let s2 = cx.subscribe(&input, |this: &mut Composer, _, ev: &TextAreaEvent, cx| {
      this.picker = picker::detect(&ev.0, &this.rt, &this.chat, cx);
      cx.notify();
    });
    let mut c = Self { rt, chat, thread, pane, input, files: Vec::new(), picker: None, editing: None, dictation: None, _subs: vec![s1, s2] };
    c.restore(cx);
    c
  }

  fn restore(&mut self, cx: &mut Context<Self>) {
    if self.thread.is_some() {
      return;
    }
    let rt = self.rt.clone();
    let id = self.chat.clone();
    cx.spawn(async move |this, cx| {
      let draft = tk::run(async move { Ok(store::chats::get(&rt.pool, &id).await?.draft) }).await.unwrap_or_default();
      let _ = this.update(cx, |this, cx| {
        if !draft.is_empty() && this.text(cx).is_empty() {
          this.input.update(cx, |i, cx| i.set_text(&draft, cx));
        }
      });
    })
    .detach();
  }

  pub fn text(&self, cx: &gpui::App) -> String {
    self.input.read(cx).text()
  }

  pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    window.focus(&self.input.read(cx).focus_handle(), cx);
  }

  pub fn escape(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    if self.picker.is_some() {
      self.picker = None;
    } else if self.editing.is_some() {
      self.editing = None;
      self.input.update(cx, |i, cx| i.set_text("", cx));
    }
    cx.notify();
  }

  /// Load a sent message back for editing.
  pub fn edit(&mut self, id: &str, body: &str, cx: &mut Context<Self>) {
    self.editing = Some(id.to_string());
    self.input.update(cx, |i, cx| i.set_text(body, cx));
    cx.notify();
  }

  pub fn prefill(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
    self.input.update(cx, |i, cx| i.set_text(text, cx));
    self.focus(window, cx);
  }

  pub fn add_files(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
    for p in paths {
      if self.files.len() >= agent::api::attach::MAX_FILES {
        self.toast(t("You can attach up to 6 files at once."), cx);
        break;
      }
      match agent::api::attach::check(&p) {
        Ok(_) => {
          if !self.files.contains(&p) {
            self.files.push(p);
          }
        }
        Err(e) => self.toast(e.to_string(), cx),
      }
    }
    cx.notify();
  }

  fn toast(&self, msg: impl Into<String>, cx: &mut Context<Self>) {
    let msg = msg.into();
    let _ = self.pane.update(cx, |p, cx| p.toast(msg, cx));
  }

  pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(p) = self.picker.take() {
      if let Some(choice) = p.items.first().cloned() {
        self.insert(&p, &choice, window, cx);
        return;
      }
    }
    let text = self.text(cx);
    if text.trim().is_empty() && self.files.is_empty() {
      return;
    }
    let files = std::mem::take(&mut self.files);
    self.input.update(cx, |i, cx| i.set_text("", cx));
    let rt = self.rt.clone();
    let chat = self.chat.clone();
    let thread = self.thread.clone();
    let editing = self.editing.take();
    cx.spawn(async move |this, cx| {
      let r = tk::run(async move {
        match editing {
          Some(id) => agent::api::chat::edit(&rt, &id, &text).await.map(|_| ()),
          None => agent::api::chat::send(&rt, &chat, &text, &files, thread.as_deref()).await.map(|_| ()),
        }
      })
      .await;
      if let Err(e) = r {
        let _ = this.update(cx, |this, cx| this.toast(e.to_string(), cx));
      }
    })
    .detach();
    cx.notify();
  }

  pub fn insert(&mut self, p: &picker::Picker, choice: &picker::Choice, window: &mut Window, cx: &mut Context<Self>) {
    let text = self.text(cx);
    let cut = text.len().saturating_sub(p.token.len());
    let new = format!("{}{}{} ", &text[..cut], p.sigil, choice.insert);
    self.input.update(cx, |i, cx| i.set_text(&new, cx));
    self.picker = None;
    self.focus(window, cx);
    cx.notify();
  }

  fn attach(&mut self, cx: &mut Context<Self>) {
    let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: Some(t("Attach").into()) });
    cx.spawn(async move |this, cx| {
      if let Ok(Ok(Some(paths))) = rx.await {
        let _ = this.update(cx, |this, cx| this.add_files(paths, cx));
      }
    })
    .detach();
  }

  /// ⌘V with an image or copied files on the clipboard attaches them;
  /// text pastes normally. Returns whether it handled the paste.
  pub fn paste_files(&mut self, cx: &mut Context<Self>) -> bool {
    let Some(item) = cx.read_from_clipboard() else { return false };
    let mut files = Vec::new();
    for e in item.entries() {
      match e {
        gpui::ClipboardEntry::Image(img) => {
          let ext = match img.format {
            gpui::ImageFormat::Jpeg => "jpg",
            gpui::ImageFormat::Gif => "gif",
            gpui::ImageFormat::Webp => "webp",
            gpui::ImageFormat::Tiff => "tiff",
            _ => "png",
          };
          let dir = std::env::temp_dir().join("asylum-paste");
          let _ = std::fs::create_dir_all(&dir);
          let path = dir.join(format!("pasted-{}.{ext}", store::now()));
          if std::fs::write(&path, &img.bytes).is_ok() {
            files.push(path);
          }
        }
        gpui::ClipboardEntry::ExternalPaths(p) => files.extend(p.paths().iter().cloned()),
        _ => {}
      }
    }
    if files.is_empty() {
      return false;
    }
    self.add_files(files, cx);
    true
  }

  pub fn dictate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if !self.rt.settings().voice_enabled {
      return;
    }
    dictate::toggle(self, window, cx);
  }
}

impl Render for Composer {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let empty = self.text(cx).trim().is_empty() && self.files.is_empty();
    let busy = self.pane.upgrade().is_some_and(|p| p.read(cx).busy());
    let group = self.pane.upgrade().is_some_and(|p| p.read(cx).is_group());
    let recording = self.dictation.as_ref().is_some_and(|d| d.recording);
    let mut chips = div().flex().flex_wrap().gap(px(6.0));
    for (i, f) in self.files.iter().enumerate() {
      let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
      chips = chips.child(
        div()
          .flex()
          .items_center()
          .gap(px(4.0))
          .pl(px(8.0))
          .pr(px(2.0))
          .py(px(2.0))
          .rounded(px(6.0))
          .bg(ink.hover)
          .text_size(px(12.0))
          .child(guise::Icon::new(IconName::Paperclip).size(Size::Xs))
          .child(SharedString::from(name))
          .child(ActionIcon::new(SharedString::from(format!("rmf{i}")), IconName::X).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(move |this, _, _, cx| {
            if i < this.files.len() {
              this.files.remove(i);
            }
            cx.notify();
          }))),
      );
    }
    let mut tools = div().flex().items_center().gap(px(2.0));
    tools = tools.child(
      div().id("attach-tip").tooltip(guise::tooltip(t("Attach files"))).child(
        ActionIcon::new("attach", IconName::Paperclip).variant(Variant::Subtle).on_click(cx.listener(|this, _, _, cx| this.attach(cx))),
      ),
    );
    let voice = self.rt.settings().voice_enabled;
    tools = tools.when(voice, |tools| tools.child(
      div().id("dictate-tip").tooltip(guise::tooltip(format!("{} (⌘D)", t("Start voice input")))).child(
        ActionIcon::new("dictate", if recording { IconName::MicOff } else { IconName::Mic })
          .variant(if recording { Variant::Filled } else { Variant::Subtle })
          .color(if recording { guise::ColorName::Red } else { guise::ColorName::Gray })
          .on_click(cx.listener(|this, _, w, cx| this.dictate(w, cx))),
      ),
    ));
    let action = if busy && empty {
      div().id("stop-tip").tooltip(guise::tooltip(t("Stop"))).child(
        ActionIcon::new("stop", IconName::Square).variant(Variant::Filled).color(guise::ColorName::Red).on_click(|_, w, cx| w.dispatch_action(Box::new(crate::actions::StopBot), cx)),
      )
    } else if voice && empty && !group && self.thread.is_none() {
      div().id("voice-tip").tooltip(guise::tooltip(t("Start voice chat"))).child(
        ActionIcon::new("voice", IconName::AudioLines).variant(Variant::Filled).on_click(cx.listener(|this, _, w, cx| {
          let pane = this.pane.clone();
          let _ = pane.update(cx, |p, cx| crate::voicechat::start(p, w, cx));
        })),
      )
    } else {
      div().id("send-tip").tooltip(guise::tooltip(t("Send"))).child(ActionIcon::new("send", IconName::ArrowUp).variant(Variant::Filled).on_click(cx.listener(|this, _, w, cx| this.submit(w, cx))))
    };
    let mut col = div()
      .id("composer")
      .relative()
      .flex()
      .flex_col()
      .gap(px(8.0))
      .p(px(10.0))
      .rounded(px(16.0))
      .border_1()
      .border_color(ink.border)
      .bg(ink.surface)
      .capture_key_down(cx.listener(|this, ev: &gpui::KeyDownEvent, _, cx| {
        if ev.keystroke.modifiers.platform && ev.keystroke.key == "v" && this.paste_files(cx) {
          cx.stop_propagation();
        }
      }))
      // Hold ⌘D to talk: releasing after a moment stops; a quick tap toggles.
      .capture_key_up(cx.listener(|this, ev: &gpui::KeyUpEvent, w, cx| {
        if ev.keystroke.key == "d" && this.dictation.as_ref().is_some_and(|d| d.recording && d.started.elapsed() > dictate::HOLD) {
          this.dictate(w, cx);
        }
      }))
      .drag_over::<ExternalPaths>(move |s, _, _, _| s.border_color(ink.primary).bg(ink.primary.opacity(0.06)))
      .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| this.add_files(paths.paths().to_vec(), cx)));
    if let Some(e) = &self.editing {
      let _ = e;
      col = col.child(
        div()
          .flex()
          .justify_between()
          .text_size(px(12.0))
          .text_color(ink.primary)
          .child(t("Editing a sent message. Sending replaces it and everything after."))
          .child(div().id("cancel-edit").cursor_pointer().child(t("Cancel")).on_click(cx.listener(|this, _, w, cx| this.escape(w, cx)))),
      );
    }
    if let Some(d) = &self.dictation {
      col = col.child(dictate::render(d, cx));
    }
    if !self.files.is_empty() {
      col = col.child(chips);
    }
    col = col
      .child(self.input.clone())
      .child(div().flex().items_center().justify_between().child(tools).child(action));
    if let Some(p) = self.picker.clone() {
      col = col.child(picker::render(self, &p, window, cx));
    }
    col
  }
}
