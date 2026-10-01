//! The computer panel: the Bot's screen (a live preview you can take over),
//! the shared workspace's files, and a terminal on the computer. Teach a
//! task records what you do on the screen and drafts a skill from it.

pub mod files;
pub mod screen;
pub mod teach;
pub mod terminal;

use crate::i18n::t;
use crate::root::Root;
use crate::theme::ink;
use crate::tk;
use agent::{Event, Runtime};
use gpui::prelude::*;
use gpui::{div, px, Bounds, Context, Entity, FocusHandle, Pixels, SharedString, WeakEntity, Window};
use guise::{ActionIcon, Button, IconName, Size, TextInput, TextInputEvent, Variant};
use libsinclair::termview::TermView;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
  Screen,
  Files,
  Terminal,
}

pub struct Panel {
  pub rt: Runtime,
  pub bot: String,
  pub root: WeakEntity<Root>,
  pub tab: Tab,
  pub shot: Option<Arc<gpui::Image>>,
  pub url: Entity<TextInput>,
  pub page: String,
  pub taking: bool,
  pub bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
  pub focus: FocusHandle,
  pub recording: bool,
  pub teach: Option<teach::Form>,
  pub started: Option<std::time::Instant>,
  pub term: Option<Entity<TermView>>,
  pub dir: String,
  pub entries: Vec<computer::fs::Entry>,
  pub state: String,
  pub status: Option<String>,
  pub alive: bool,
  pub _subs: Vec<gpui::Subscription>,
}

impl Panel {
  pub fn new(rt: Runtime, bot: String, root: WeakEntity<Root>, state: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let url = cx.new(|cx| TextInput::new(cx).placeholder(t("Go to a website")).size(Size::Xs));
    let sub = cx.subscribe_in(&url, window, |this: &mut Panel, _, ev: &TextInputEvent, _, cx| {
      if let TextInputEvent::Submit(u) = ev {
        this.navigate(u.clone(), cx);
      }
    });
    let mut p = Self {
      rt,
      bot,
      root,
      tab: Tab::Screen,
      shot: None,
      url,
      page: String::new(),
      taking: false,
      bounds: Rc::new(Cell::new(None)),
      focus: cx.focus_handle(),
      recording: false,
      teach: None,
      started: None,
      term: None,
      dir: "~".into(),
      entries: Vec::new(),
      state,
      status: None,
      alive: true,
      _subs: vec![sub],
    };
    p.recording_state(cx);
    p.poll(cx);
    p.list(cx);
    p
  }

  fn recording_state(&mut self, cx: &mut Context<Self>) {
    let rt = self.rt.clone();
    let bot = self.bot.clone();
    cx.spawn(async move |this, cx| {
      let rec = tk::run(async move { Ok(agent::teach::recording(&rt, &bot).await) }).await.ok().flatten();
      let _ = this.update(cx, |this, cx| {
        this.recording = rec.is_some();
        cx.notify();
      });
    })
    .detach();
  }

  /// Refresh the screen preview about once a second while the panel lives.
  fn poll(&mut self, cx: &mut Context<Self>) {
    cx.spawn(async move |this, cx| loop {
      let Ok((rt, bot, tab)) = this.update(cx, |p, _| (p.rt.clone(), p.bot.clone(), p.tab)) else { break };
      if tab == Tab::Screen {
        let got = tk::run(async move {
          match rt.browser.url(&bot).await {
            Some(url) => Ok(Some((url, rt.browser.screenshot(&bot).await?))),
            None => Ok(None),
          }
        })
        .await
        .ok()
        .flatten();
        let alive = this.update(cx, |p, cx| {
          if let Some((url, png)) = got {
            p.page = url;
            p.shot = Some(Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, png)));
          }
          cx.notify();
          p.alive
        });
        if !matches!(alive, Ok(true)) {
          break;
        }
      }
      let taking = this.update(cx, |p, _| p.taking).unwrap_or(false);
      let ms = if taking { 350 } else { 1000 };
      cx.background_executor().timer(std::time::Duration::from_millis(ms)).await;
      // Auto-stop a demo at ten minutes.
      let _ = this.update(cx, |p, cx| {
        if p.recording && p.started.is_some_and(|s| s.elapsed().as_millis() as i64 >= agent::teach::MAX_MS) {
          teach::stop(p, cx);
        }
      });
    })
    .detach();
  }

  pub fn navigate(&mut self, url: String, cx: &mut Context<Self>) {
    if url.trim().is_empty() {
      return;
    }
    let rt = self.rt.clone();
    let bot = self.bot.clone();
    cx.spawn(async move |this, cx| {
      let r = tk::run(async move { rt.browser.open(&bot, &url).await.map(|_| ()) }).await;
      let _ = this.update(cx, |p, cx| {
        p.status = r.err().map(|e| e.to_string());
        cx.notify();
      });
    })
    .detach();
  }

  pub fn browse(&mut self, f: fn(agent::Runtime, String) -> futures::future::BoxFuture<'static, anyhow::Result<()>>, cx: &mut Context<Self>) {
    let (rt, bot) = (self.rt.clone(), self.bot.clone());
    tk::spawn(async move {
      let _ = f(rt, bot).await;
    });
    cx.notify();
  }

  pub fn on_event(&mut self, e: &Event, cx: &mut Context<Self>) {
    match e {
      Event::Computer { state } if !state.starts_with("disk:") => {
        self.state = state.clone();
        cx.notify();
      }
      Event::Screen { bot } if bot == &self.bot => {
        self.recording_state(cx);
        cx.notify();
      }
      Event::Message { .. } if self.tab == Tab::Files => self.list(cx),
      _ => {}
    }
  }

  pub fn toggle_teach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.recording {
      teach::stop(self, cx);
    } else {
      self.tab = Tab::Screen;
      self.teach = Some(teach::Form::new(window, cx));
    }
    cx.notify();
  }

  /// Take control of the Bot's screen (from an "Action needed" card).
  pub fn takeover(&mut self, on: bool, bot: &str, cx: &mut Context<Self>) {
    if bot != self.bot {
      return;
    }
    self.tab = Tab::Screen;
    self.taking = on;
    cx.notify();
  }

  /// "I'm done": hand control back to the Bot.
  pub fn done(&mut self, cx: &mut Context<Self>) {
    self.taking = false;
    let rt = self.rt.clone();
    let bot = self.bot.clone();
    tk::spawn(async move { agent::api::cards::takeover(&rt, &bot, true).await });
    cx.notify();
  }
}

type Fix = (&'static str, fn(&mut Panel, &mut Context<Panel>));

fn state_banner(p: &Panel, cx: &mut Context<Panel>) -> Option<gpui::AnyElement> {
  let ink = ink(cx);
  let (text, action): (&str, Option<Fix>) = match p.state.as_str() {
    "starting" => ("Starting your computer", None),
    "updating" => ("Updating your computer", None),
    "recovering" | "resetting" => ("Reconnecting", None),
    "recreating" => ("Recreating your computer. Agents resume when it's ready.", None),
    "hibernating" => (
      "Your computer is hibernating to save memory. It wakes when an Agent needs it.",
      Some(("Wake", |p, cx| {
        let rt = p.rt.clone();
        tk::spawn(async move { agent::api::computer::wake(&rt).await });
        cx.notify();
      })),
    ),
    "unreachable" => (
      "Couldn't reach the computer",
      Some(("Retry", |p, cx| {
        let rt = p.rt.clone();
        tk::spawn(async move { agent::api::computer::start(&rt).await });
        cx.notify();
      })),
    ),
    _ => return None,
  };
  let mut row = div().flex().items_center().gap(px(8.0)).p(px(10.0)).rounded(px(8.0)).bg(ink.warning.opacity(0.12)).text_size(px(12.5)).child(t(text));
  if let Some((label, f)) = action {
    row = row.child(div().flex_1()).child(Button::new("state-action", t(label)).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| f(this, cx))));
  }
  if p.state == "unreachable" {
    row = row
      .child(Button::new("recover", t("Recover computer")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(|this, _, _, cx| {
        let rt = this.rt.clone();
        this.status = Some(t("Recovering…").into());
        tk::spawn(async move { agent::api::computer::recover(&rt).await });
        cx.notify();
      })));
  }
  Some(row.into_any_element())
}

impl Panel {
  /// Switch to the Terminal tab (⌘⇧T).
  pub fn show_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.tab = Tab::Terminal;
    terminal::ensure(self, window, cx);
    cx.notify();
  }
}

impl Render for Panel {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let root = self.root.clone();
    let disk = self.root.upgrade().map(|r| r.read(cx).disk.clone()).unwrap_or_default();
    let mut tabs = div().flex().gap(px(4.0));
    for (tab, label, icon) in [(Tab::Screen, "Screen", IconName::Monitor), (Tab::Files, "Files", IconName::Folder), (Tab::Terminal, "Terminal", IconName::Terminal)] {
      let on = self.tab == tab;
      tabs = tabs.child(
        div()
          .id(label)
          .flex()
          .items_center()
          .gap(px(6.0))
          .px(px(10.0))
          .py(px(4.0))
          .rounded(px(6.0))
          .text_size(px(12.5))
          .cursor_pointer()
          .when(on, |d| d.bg(ink.hover).font_weight(gpui::FontWeight::SEMIBOLD))
          .hover(|s| s.bg(ink.hover))
          .on_click(cx.listener(move |this, _, w, cx| {
            this.tab = tab;
            if tab == Tab::Files {
              this.list(cx);
            }
            if tab == Tab::Terminal {
              terminal::ensure(this, w, cx);
            }
            cx.notify();
          }))
          .child(guise::Icon::new(icon).size(Size::Xs))
          .child(t(label)),
      );
    }
    let body = match self.tab {
      Tab::Screen => screen::render(self, window, cx),
      Tab::Files => files::render(self, cx),
      Tab::Terminal => terminal::render(self, cx),
    };
    let mut col = div()
      .size_full()
      .flex()
      .flex_col()
      .child(
        div()
          .h(px(56.0))
          .flex_none()
          .flex()
          .items_center()
          .justify_between()
          .px(px(16.0))
          .border_b_1()
          .border_color(ink.border)
          .child(div().flex().items_center().gap(px(8.0)).child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(t("Computer"))).child(tabs))
          .child(ActionIcon::new("close-computer", IconName::X).variant(Variant::Subtle).on_click(move |_, _, cx| {
            let _ = root.update(cx, |r, cx| r.close_right(cx));
          })),
      );
    let mut inner = div().flex_1().min_h_0().flex().flex_col().gap(px(10.0)).p(px(14.0));
    if let Some(b) = state_banner(self, cx) {
      inner = inner.child(b);
    }
    if disk == "low" || disk == "critical" {
      inner = inner.child(
        div()
          .flex()
          .items_center()
          .gap(px(8.0))
          .p(px(10.0))
          .rounded(px(8.0))
          .bg(if disk == "critical" { ink.danger.opacity(0.12) } else { ink.warning.opacity(0.12) })
          .text_size(px(12.5))
          .child(if disk == "critical" { t("Computer is critically low on disk space") } else { t("Computer is low on disk space") })
          .child(div().flex_1())
          .child(Button::new("disk-saver", t("Go to Disk Saver")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, _, cx| {
            let rt = this.rt.clone();
            let root = this.root.clone();
            cx.spawn(async move |_, cx| {
              if let Ok((_, c)) = tk::run(async move { agent::api::bots::disk_saver(&rt).await }).await {
                let _ = root.update(cx, |r, cx| {
                  r.reload(cx);
                  r.pending_open(c.id, cx);
                });
              }
            })
            .detach();
          }))),
      );
    }
    inner = inner.child(body);
    if let Some(s) = &self.status {
      inner = inner.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(SharedString::from(s.clone())));
    }
    col = col.child(inner);
    col
  }
}
