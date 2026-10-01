//! The main window: title bar, sidebar, the open conversation, the right
//! pane (details or computer), and overlays (dialogs, palette, menus,
//! toasts). Engine events arrive here and fan out.

pub mod actions;
pub mod dialogs;
pub mod events;
pub mod nav;
pub mod problem;
pub mod titlebar;

use crate::chat::ChatPane;
use crate::state::{Item, Snap};
use crate::theme::ink;
use crate::tk;
use agent::Runtime;
use gpui::prelude::*;
use gpui::{div, px, AnyView, App, Context, Entity, FocusHandle, Focusable, Subscription, Window};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Right {
  None,
  Details,
  Computer,
  Web,
}

pub struct Root {
  pub rt: Runtime,
  pub snap: Snap,
  pub active: Option<String>,
  pub pane: Option<Entity<ChatPane>>,
  pub right: Right,
  pub compact: bool,
  pub modal: Option<AnyView>,
  pub spotlight: Option<Entity<guise::Spotlight>>,
  pub menu: Entity<guise::ContextMenu>,
  pub toasts: Entity<guise::ToastStack>,
  pub history: nav::History,
  pub focus: FocusHandle,
  pub computer: String,
  pub disk: String,
  pub rename: Option<(Item, Entity<guise::TextInput>)>,
  pub details: Option<Entity<crate::details::Details>>,
  pub panel: Option<Entity<crate::computer::Panel>>,
  /// The built-in browser; kept across chats so its page survives.
  pub web: Option<Entity<crate::web::Browser>>,
  pub voice: Option<Entity<crate::voicechat::Call>>,
  pub market: Option<Entity<crate::market::Market>>,
  pub show_hidden: bool,
  pub zoom: f32,
  pub pending: Option<String>,
  pub pending_item: Option<Item>,
  pub onboard: bool,
  /// The computer panel fills the content area (full takeover view).
  pub wide: bool,
  pub startup: Option<String>,
  pub _subs: Vec<Subscription>,
}

impl Focusable for Root {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus.clone()
  }
}

impl Root {
  pub fn new(rt: Runtime, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let settings = rt.settings();
    let appearance = cx.observe_window_appearance(window, |this: &mut Root, window, cx| {
      let dark = crate::theme::dark_for(this.rt.settings().appearance, window.appearance());
      crate::theme::install(dark, cx);
      cx.refresh_windows();
    });
    let activation = cx.observe_window_activation(window, |this: &mut Root, window, cx| {
      if window.is_window_active() {
        let rt = this.rt.clone();
        tk::spawn(async move { agent::ticker::seen(&rt).await });
      }
      let _ = cx;
    });
    let menu = cx.new(guise::ContextMenu::new);
    let toasts = cx.new(|_| guise::ToastStack::new().duration(Some(std::time::Duration::from_secs(4))));
    window.set_rem_size(px(16.0 * settings.zoom));
    let mut root = Self {
      rt,
      snap: Snap::default(),
      active: None,
      pane: None,
      right: Right::None,
      web: None,
      compact: settings.compact_sidebar,
      modal: None,
      spotlight: None,
      menu,
      toasts,
      history: nav::History::default(),
      focus: cx.focus_handle(),
      computer: String::new(),
      disk: "ok".into(),
      rename: None,
      details: None,
      panel: None,
      voice: None,
      market: None,
      show_hidden: false,
      zoom: settings.zoom,
      pending: None,
      pending_item: None,
      onboard: false,
      wide: false,
      startup: None,
      _subs: vec![appearance, activation],
    };
    root.reload(cx);
    if std::env::var("ASYLUM_SHOW").is_err() {
      crate::onboarding::check(&mut root, cx);
      crate::settings::required::check(&mut root, cx);
    }
    root
  }

  /// Re-read the sidebar snapshot; opens the first chat on first load.
  pub fn reload(&mut self, cx: &mut Context<Self>) {
    let rt = self.rt.clone();
    cx.spawn(async move |this, cx| {
      let snap = match tk::run(crate::state::load(rt)).await {
        Ok(s) => s,
        Err(e) => {
          eprintln!("could not load the sidebar: {e:#}");
          let _ = this.update(cx, |this, cx| this.toast(format!("{} {e}", crate::i18n::t("Couldn't load your Agents.")), cx));
          return;
        }
      };
      let _ = this.update(cx, |this, cx| {
        this.snap = snap;
        crate::notify::badge(this.snap.unread() + this.snap.approvals.len());
        cx.notify();
      });
    })
    .detach();
  }

  pub fn toast(&self, msg: impl Into<String>, cx: &mut Context<Self>) {
    let msg: String = msg.into();
    self.toasts.update(cx, |t, cx| {
      t.push(msg, cx);
    });
  }

  /// Run an engine call and toast its error, if any.
  pub fn run<F, T>(&mut self, cx: &mut Context<Self>, f: F, then: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static)
  where
    F: std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
    T: Send + 'static,
  {
    cx.spawn(async move |this, cx| {
      let r = tk::run(f).await;
      let _ = this.update(cx, |this, cx| match r {
        Ok(v) => then(this, v, cx),
        Err(e) => {
          if !problem::show(this, &e.to_string(), cx) {
            this.toast(e.to_string(), cx);
          }
        }
      });
    })
    .detach();
  }

  /// Open a conversation.
  pub fn open(&mut self, chat: &str, window: &mut Window, cx: &mut Context<Self>) {
    if self.active.as_deref() == Some(chat) {
      if let Some(p) = &self.pane {
        p.update(cx, |p, cx| p.focus_composer(window, cx));
      }
      return;
    }
    if let Some(p) = &self.pane {
      p.update(cx, |p, cx| p.leave(cx));
    }
    self.history.visit(self.active.as_deref(), chat);
    self.active = Some(chat.to_string());
    let rt = self.rt.clone();
    let root = cx.entity().downgrade();
    let id = chat.to_string();
    self.pane = Some(cx.new(|cx| ChatPane::new(rt.clone(), id.clone(), root, window, cx)));
    self.details = None;
    self.panel = None;
    if self.right == Right::Details {
      self.show_details(window, cx);
    } else if self.right == Right::Computer {
      self.show_computer(window, cx);
    }
    let c = chat.to_string();
    self.run(cx, async move { agent::api::chat::open(&rt, &c).await }, |_, _, _| {});
    cx.notify();
  }

  /// Open a chat on the next frame (from async callbacks without a window).
  pub fn pending_open(&mut self, chat: String, cx: &mut Context<Self>) {
    self.pending = Some(chat);
    cx.notify();
  }

  pub fn pending_open_item(&mut self, item: Item, cx: &mut Context<Self>) {
    self.pending_item = Some(item);
    self.reload(cx);
  }

  pub fn open_item(&mut self, item: &Item, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(c) = self.snap.chat_for(item).map(|c| c.id.clone()) {
      self.open(&c, window, cx);
    }
  }

  pub fn active_item(&self) -> Option<Item> {
    self.active.as_deref().and_then(|c| self.snap.item_of(c))
  }

  /// The Bot of the open 1:1 conversation.
  pub fn active_bot(&self) -> Option<store::Bot> {
    match self.active_item()? {
      Item::Bot(b) => self.snap.bot(&b).cloned(),
      Item::Group(_) => None,
    }
  }

  pub fn show_details(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let Some(chat) = self.active.clone() else { return };
    self.right = Right::Details;
    let rt = self.rt.clone();
    let root = cx.entity().downgrade();
    self.details = Some(cx.new(|cx| crate::details::Details::new(rt, chat, root, window, cx)));
    self.panel = None;
    self.hide_web(cx);
    cx.notify();
  }

  pub fn show_computer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let Some(bot) = self.active_bot() else {
      self.toast(crate::i18n::t("Open an Agent's chat to see its computer."), cx);
      return;
    };
    self.right = Right::Computer;
    let rt = self.rt.clone();
    let root = cx.entity().downgrade();
    let state = self.computer.clone();
    self.panel = Some(cx.new(|cx| crate::computer::Panel::new(rt, bot.id.clone(), root, state, window, cx)));
    self.details = None;
    self.hide_web(cx);
    cx.notify();
  }

  /// Open the built-in browser at a URL (or keep its page when `None`).
  pub fn show_web(&mut self, url: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
    match (&self.web, url) {
      (Some(w), Some(u)) => w.update(cx, |w, cx| w.go(u, cx)),
      (Some(_), None) => {}
      (None, start) => {
        let root = cx.entity().downgrade();
        let ws = self.rt.computer.workspace();
        let start = start.unwrap_or("about:blank").to_string();
        self.web = Some(cx.new(|cx| crate::web::Browser::new(root, ws, &start, window, cx)));
      }
    }
    self.right = Right::Web;
    self.details = None;
    self.panel = None;
    cx.notify();
  }

  /// Open a workspace file (HTML, images, PDFs) in the built-in browser.
  pub fn preview_file(&mut self, relative: &str, window: &mut Window, cx: &mut Context<Self>) {
    let url = crate::web::address::workspace(relative.trim_start_matches("~/"));
    self.show_web(Some(&url), window, cx);
  }

  pub fn close_right(&mut self, cx: &mut Context<Self>) {
    if self.right == Right::Web {
      if let Some(w) = &self.web {
        w.update(cx, |w, cx| w.set_visible(false, cx));
      }
    }
    self.right = Right::None;
    self.wide = false;
    self.details = None;
    self.panel = None;
    cx.notify();
  }

  fn hide_web(&mut self, cx: &mut Context<Self>) {
    if let Some(w) = &self.web {
      w.update(cx, |w, cx| w.set_visible(false, cx));
    }
  }

  pub fn set_modal(&mut self, view: impl Into<AnyView>, cx: &mut Context<Self>) {
    self.modal = Some(view.into());
    cx.notify();
  }

  pub fn close_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.modal = None;
    if let Some(p) = &self.pane {
      p.update(cx, |p, cx| p.focus_composer(window, cx));
    }
    cx.notify();
  }
}

impl Render for Root {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    if let Some(chat) = self.pending.take() {
      self.open(&chat, window, cx);
    }
    if self.onboard && self.modal.is_none() {
      self.onboard = false;
      crate::onboarding::open(self, window, cx);
    }
    if let Some(item) = self.pending_item.clone() {
      if self.snap.chat_for(&item).is_some() {
        self.pending_item = None;
        self.open_item(&item, window, cx);
        match self.startup.take().as_deref() {
          Some("details") => self.show_details(window, cx),
          Some("computer") => {
            self.show_computer(window, cx);
            if let (Ok(url), Some(p)) = (std::env::var("ASYLUM_URL"), self.panel.clone()) {
              p.update(cx, |p, cx| p.navigate(url, cx));
            }
          }
          _ => {}
        }
      }
    }
    let ink = ink(cx);
    let main = match &self.pane {
      Some(p) => div().flex_1().min_w_0().h_full().child(p.clone()).into_any_element(),
      None => crate::chat::empty::render(self, window, cx).into_any_element(),
    };
    let right = match self.right {
      Right::Details => self.details.clone().map(|d| d.into_any_element()),
      Right::Computer => self.panel.clone().map(|p| p.into_any_element()),
      Right::Web => self.web.clone().map(|w| w.into_any_element()),
      Right::None => None,
    };
    // The web view is a native surface drawn over gpui: keep it out of the
    // way of dialogs, and let it show again when they close.
    if let Some(w) = self.web.clone() {
      let show = self.right == Right::Web && self.modal.is_none();
      w.update(cx, |w, cx| w.set_visible(show, cx));
    }
    let wide = self.wide && matches!(self.right, Right::Computer | Right::Web);
    let mut body = div().flex().flex_1().min_h_0().child(crate::sidebar::render(self, window, cx));
    if !wide {
      body = body.child(main);
    }
    if let Some(r) = right {
      body = body.child(
        div()
          .when(wide, |d| d.flex_1())
          // The browser shares the width with the chat; the others are fixed.
          .when(!wide && self.right == Right::Web, |d| d.flex_1().min_w_0())
          .when(!wide && self.right != Right::Web, |d| d.w(px(if self.right == Right::Computer { 520.0 } else { 360.0 })).flex_none())
          .h_full()
          .border_l_1()
          .border_color(ink.border)
          .bg(ink.body)
          .child(r),
      );
    }
    let mut root = div()
      .id("root")
      .key_context(crate::actions::CONTEXT)
      .track_focus(&self.focus)
      .size_full()
      .flex()
      .flex_col()
      .bg(ink.body)
      .text_color(ink.text)
      .text_size(px(14.0))
      .font_family(".SystemUIFont")
      .child(titlebar::render(self, window, cx))
      .child(body);
    root = actions::wire(root, cx);
    if let Some(v) = &self.voice {
      root = root.child(v.clone());
    }
    if let Some(m) = &self.modal {
      root = root.child(m.clone());
    }
    if let Some(s) = &self.spotlight {
      root = root.child(s.clone());
    }
    root.child(self.menu.clone()).child(div().absolute().bottom(px(16.0)).right(px(16.0)).child(self.toasts.clone()))
  }
}

/// Open the main window.
pub fn open(rt: Runtime, cx: &mut App) -> anyhow::Result<gpui::WindowHandle<Root>> {
  use gpui::{point, size, Bounds, TitlebarOptions, WindowBounds, WindowOptions};
  let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
  let options = WindowOptions {
    window_bounds: Some(WindowBounds::Windowed(bounds)),
    window_min_size: Some(size(px(720.0), px(480.0))),
    titlebar: Some(TitlebarOptions {
      title: Some("Asylum".into()),
      appears_transparent: true,
      traffic_light_position: Some(point(px(14.0), px(13.0))),
    }),
    app_id: Some("io.wess.asylum".into()),
    ..Default::default()
  };
  let handle = cx.open_window(options, |window, cx| {
    let dark = crate::theme::dark_for(rt.settings().appearance, window.appearance());
    crate::theme::install(dark, cx);
    cx.new(|cx| Root::new(rt.clone(), window, cx))
  })?;
  handle
    .update(cx, |root, window, cx| {
      window.focus(&root.focus_handle(cx), cx);
      show(root, window, cx);
    })
    .ok();
  Ok(handle)
}

/// Dev hook for screenshots without driving input: `ASYLUM_SHOW` names a
/// screen to open at launch — `settings:<page>`, `market`, `new`, `palette`,
/// `about`, `web[:<url>]`, or `bot:<name>[:details|:computer]`.
fn show(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
  let Ok(spec) = std::env::var("ASYLUM_SHOW") else { return };
  // `web:<url or workspace/path>` opens the built-in browser.
  if let Some(rest) = spec.strip_prefix("web") {
    let url = rest.strip_prefix(':').filter(|u| !u.is_empty());
    root.show_web(url, window, cx);
    return;
  }
  let mut parts = spec.splitn(3, ':');
  match (parts.next(), parts.next(), parts.next()) {
    (Some("settings"), page, _) => {
      let page: &'static str = page.and_then(|p| crate::settings::PAGES.iter().map(|(id, _, _)| *id).chain(["rules"]).find(|id| *id == p)).unwrap_or("general");
      crate::settings::open(root, Some(page), window, cx);
    }
    (Some("market"), _, _) => crate::market::open(root, window, cx),
    (Some("new"), _, _) => crate::newchat::open(root, window, cx),
    (Some("palette"), _, _) => crate::palette::open(root, None, window, cx),
    (Some("about"), _, _) => crate::settings::about(root, window, cx),
    (Some("onboarding"), _, _) => crate::onboarding::open(root, window, cx),
    (Some("bot"), Some(name), pane) => {
      let rt = root.rt.clone();
      let name = name.to_string();
      let pane = pane.map(str::to_string);
      root.run(cx, async move { Ok(store::bots::find(&rt.pool, &name).await?.map(|b| b.id)) }, move |r, id, cx| {
        if let Some(id) = id {
          r.startup = pane.clone();
          r.pending_open_item(crate::state::Item::Bot(id), cx);
        }
      });
    }
    _ => {}
  }
}
