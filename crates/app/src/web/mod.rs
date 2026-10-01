//! The built-in browser: a native web view (guise `WebView`, WKWebView on
//! macOS) in the right-hand pane, for links from chats and previews of
//! workspace files. It's the user's browser, not the Agents' computer — Agents
//! keep driving their own Chromium, where sign-ins are shared between them.

pub mod address;

use crate::i18n::t;
use crate::root::Root;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, Context, Entity, SharedString, Subscription, WeakEntity, Window};
use guise::{ActionIcon, IconName, Size, TextInput, TextInputEvent, Variant, WebView, WebViewEvent};
use std::path::PathBuf;

/// Files the built-in browser shows well.
pub const PREVIEW: [&str; 12] = ["html", "htm", "svg", "pdf", "png", "jpg", "jpeg", "gif", "webp", "txt", "json", "md"];

pub fn previewable(path: &str) -> bool {
  path.rsplit_once('.').is_some_and(|(_, ext)| PREVIEW.contains(&ext.to_ascii_lowercase().as_str()))
}

pub struct Browser {
  pub root: WeakEntity<Root>,
  pub view: Entity<WebView>,
  pub address: Entity<TextInput>,
  pub url: String,
  pub title: String,
  pub loading: bool,
  _subs: Vec<Subscription>,
}

impl Browser {
  pub fn new(root: WeakEntity<Root>, workspace: PathBuf, start: &str, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let first = address::resolve(start);
    // Serving the workspace gives its files a real origin (guise://localhost/…).
    let view = cx.new(|cx| WebView::new(cx).serve(workspace.clone(), "index.html").url(first.clone()).bordered(false));
    let field = cx.new(|cx| TextInput::new(cx).value(&address::shown(&first)).placeholder(t("Search or enter a website")).size(Size::Xs));
    let s1 = cx.subscribe_in(&field, window, |this: &mut Browser, _, ev: &TextInputEvent, _, cx| {
      if let TextInputEvent::Submit(text) = ev {
        let text = text.clone();
        this.go(&text, cx);
      }
    });
    let s2 = cx.subscribe(&view, |this: &mut Browser, _, ev: &WebViewEvent, cx| {
      match ev {
        WebViewEvent::UrlChanged(u) => {
          this.url = u.to_string();
          let shown = address::shown(&this.url);
          this.address.update(cx, |a, cx| a.set_text(&shown, cx));
        }
        WebViewEvent::TitleChanged(title) => this.title = title.to_string(),
        WebViewEvent::LoadStarted => this.loading = true,
        WebViewEvent::LoadFinished => this.loading = false,
        WebViewEvent::Message(_) => {}
      }
      cx.notify();
    });
    Self { root, view, address: field, url: first, title: String::new(), loading: true, _subs: vec![s1, s2] }
  }

  /// Load a URL, a workspace path, or a search.
  pub fn go(&mut self, input: &str, cx: &mut Context<Self>) {
    let url = address::resolve(input);
    self.url = url.clone();
    self.view.update(cx, |v, cx| v.load_url(url, cx));
    cx.notify();
  }

  /// The native view sits above everything gpui draws, so hide it while a
  /// dialog covers the window or when the pane closes.
  pub fn set_visible(&mut self, on: bool, cx: &mut Context<Self>) {
    self.view.update(cx, |v, _| v.set_visible(on));
  }

  fn script(&self, js: &str, cx: &mut Context<Self>) {
    self.view.read(cx).evaluate_script(js);
  }
}

impl Render for Browser {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let local = self.url.starts_with("guise://");
    let root = self.root.clone();
    let url = self.url.clone();
    let bar = div()
      .w_full()
      .min_w_0()
      .overflow_hidden()
      .flex()
      .items_center()
      .gap(px(2.0))
      .px(px(8.0))
      .py(px(6.0))
      .border_b_1()
      .border_color(ink.border)
      .child(ActionIcon::new("web-back", IconName::ArrowLeft).variant(Variant::Subtle).size(Size::Sm).on_click(cx.listener(|this, _, _, cx| this.script("history.back()", cx))))
      .child(ActionIcon::new("web-forward", IconName::ArrowRight).variant(Variant::Subtle).size(Size::Sm).on_click(cx.listener(|this, _, _, cx| this.script("history.forward()", cx))))
      .child(ActionIcon::new("web-reload", if self.loading { IconName::X } else { IconName::RotateCw }).variant(Variant::Subtle).size(Size::Sm).on_click(cx.listener(|this, _, _, cx| {
        let js = if this.loading { "window.stop()" } else { "location.reload()" };
        this.script(js, cx);
      })))
      .child(div().flex_1().min_w_0().overflow_hidden().px(px(4.0)).child(self.address.clone()));
    // Title row: the page title, then open-externally and close, which must
    // stay visible however narrow the pane gets.
    let titlebar = div()
      .w_full()
      .min_w_0()
      .flex()
      .items_center()
      .gap(px(4.0))
      .px(px(12.0))
      .py(px(2.0))
      .border_b_1()
      .border_color(ink.border)
      .child(div().flex_1().min_w_0().text_size(px(11.5)).text_color(ink.dimmed).truncate().child(SharedString::from(if self.title.is_empty() { address::shown(&self.url) } else { self.title.clone() })))
      .when(!local, |d| {
        d.child(ActionIcon::new("web-external", IconName::ExternalLink).variant(Variant::Subtle).size(Size::Xs).on_click(move |_, _, cx| cx.open_url(&url)))
      })
      .child(ActionIcon::new("web-close", IconName::X).variant(Variant::Subtle).size(Size::Xs).on_click(move |_, _, cx| {
        let _ = root.update(cx, |r, cx| r.close_right(cx));
      }));
    div()
      .size_full()
      .min_w_0()
      .overflow_hidden()
      .flex()
      .flex_col()
      .child(bar)
      .child(titlebar)
      .child(div().flex_1().min_h_0().child(self.view.clone()))
  }
}
