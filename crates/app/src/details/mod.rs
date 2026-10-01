//! The details pane for the open conversation: a Bot's settings, routines,
//! secrets, memory, skills, and sharing — or a group's name, description,
//! and members.

pub mod group;
pub mod memory;
pub mod profile;
pub mod routines;
pub mod secrets;
pub mod share;
pub mod skills;

use crate::i18n::t;
use crate::root::Root;
use crate::theme::ink;
use crate::tk;
use agent::{Event, Runtime};
use gpui::prelude::*;
use gpui::{div, px, AnyElement, App, Context, Entity, SharedString, WeakEntity, Window};
use guise::{ActionIcon, IconName, Size, TextArea, TextInput, Variant};
use store::{Bot, Chat, Memory, Routine, Run, Secret, Skill};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
  Settings,
  Tasks,
  Secrets,
  Memory,
  Skills,
  Share,
}

pub const TABS: [(Tab, &str); 6] = [
  (Tab::Settings, "Agent settings"),
  (Tab::Tasks, "Tasks"),
  (Tab::Secrets, "Secrets"),
  (Tab::Memory, "Memory"),
  (Tab::Skills, "Skills"),
  (Tab::Share, "Share"),
];

#[derive(Default, Clone)]
pub struct Data {
  pub chat: Option<Chat>,
  pub bot: Option<Bot>,
  pub members: Vec<Bot>,
  pub all: Vec<Bot>,
  pub routines: Vec<Routine>,
  pub runs: Vec<Run>,
  pub secrets: Vec<Secret>,
  pub memory: Vec<Memory>,
  pub skills: Vec<Skill>,
  pub enabled: Vec<String>,
  pub template: Option<store::Template>,
  pub slack: Option<store::slack::App>,
}

pub struct Details {
  pub rt: Runtime,
  pub chat: String,
  pub root: WeakEntity<Root>,
  pub tab: Tab,
  pub data: Data,
  pub name: Entity<TextInput>,
  pub label: Entity<TextInput>,
  pub description: Entity<TextArea>,
  pub model: Entity<TextInput>,
  pub secret_name: Entity<TextInput>,
  pub secret_desc: Entity<TextInput>,
  pub secret_value: Entity<TextInput>,
  pub slack_bot: Entity<TextInput>,
  pub slack_app: Entity<TextInput>,
  pub adding_secret: bool,
  /// Publish to Team: memories flipped from their default (facts and
  /// summaries in, preferences out), and whether routines move.
  pub team_flip: std::collections::HashSet<String>,
  pub team_routines: bool,
  pub replacing: Option<String>,
  pub routine: Option<String>,
  pub testing: Option<String>,
  pub status: Option<String>,
  pub loaded: bool,
}

impl Details {
  pub fn new(rt: Runtime, chat: String, root: WeakEntity<Root>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
    let mut d = Self {
      rt,
      chat,
      root,
      tab: Tab::Settings,
      data: Data::default(),
      name: cx.new(|cx| TextInput::new(cx).size(Size::Sm)),
      label: cx.new(|cx| TextInput::new(cx).placeholder(t("e.g. Talent Scout")).size(Size::Sm)),
      description: cx.new(|cx| TextArea::new(cx).rows(5).max_rows(14).placeholder(t("Standing job and rules, e.g. \"Never send external messages without approval.\""))),
      model: cx.new(|cx| TextInput::new(cx).placeholder(t("Default")).size(Size::Sm)),
      secret_name: cx.new(|cx| TextInput::new(cx).placeholder("API_TOKEN").size(Size::Sm)),
      secret_desc: cx.new(|cx| TextInput::new(cx).placeholder(t("What it's for")).size(Size::Sm)),
      secret_value: cx.new(|cx| TextInput::new(cx).password(true).placeholder(t("Value")).size(Size::Sm)),
      slack_bot: cx.new(|cx| TextInput::new(cx).password(true).placeholder(t("Bot token (xoxb-…)")).size(Size::Sm)),
      slack_app: cx.new(|cx| TextInput::new(cx).password(true).placeholder(t("App-level token (xapp-…)")).size(Size::Sm)),
      adding_secret: false,
      team_flip: std::collections::HashSet::new(),
      team_routines: false,
      replacing: None,
      routine: None,
      testing: None,
      status: None,
      loaded: false,
    };
    d.load(cx);
    d
  }

  pub fn load(&mut self, cx: &mut Context<Self>) {
    let rt = self.rt.clone();
    let id = self.chat.clone();
    cx.spawn(async move |this, cx| {
      let r = tk::run(async move {
        let pool = &rt.pool;
        let chat = store::chats::get(pool, &id).await?;
        let all = store::bots::list(pool).await?;
        let member_ids = store::chats::members(pool, &id).await?;
        let members: Vec<Bot> = all.iter().filter(|b| member_ids.contains(&b.id)).cloned().collect();
        let mut data = Data { chat: Some(chat.clone()), members, all, skills: store::skills::all(pool).await?, ..Default::default() };
        if let Some(bid) = &chat.bot_id {
          data.bot = Some(store::bots::get(pool, bid).await?);
          data.routines = store::routines::for_bot(pool, bid).await?;
          data.secrets = store::secrets::list(pool, bid).await?;
          data.memory = store::memories::list(pool, bid).await?;
          data.enabled = store::skills::enabled(pool, bid).await?.into_iter().map(|s| s.id).collect();
          data.template = store::templates::for_bot(pool, bid).await?;
          data.slack = store::slack::app(pool, bid).await?;
          data.runs = store::runs::for_bot(pool, bid, 50).await?;
        }
        Ok(data)
      })
      .await;
      let _ = this.update(cx, |this, cx| {
        if let Ok(data) = r {
          let first = !this.loaded;
          this.data = data;
          this.loaded = true;
          if first {
            this.fill(cx);
          }
        }
        cx.notify();
      });
    })
    .detach();
  }

  /// Put the loaded profile into the editable fields.
  fn fill(&mut self, cx: &mut Context<Self>) {
    if let Some(b) = self.data.bot.clone() {
      self.name.update(cx, |i, cx| i.set_text(&b.name, cx));
      self.label.update(cx, |i, cx| i.set_text(&b.label, cx));
      self.description.update(cx, |i, cx| i.set_text(&b.description, cx));
      let m = if b.provider.is_empty() { String::new() } else { format!("{}/{}", b.provider, b.model) };
      self.model.update(cx, |i, cx| i.set_text(&m, cx));
    } else if let Some(c) = self.data.chat.clone() {
      self.name.update(cx, |i, cx| i.set_text(&c.title, cx));
      self.description.update(cx, |i, cx| i.set_text(&c.description, cx));
    }
  }

  pub fn on_event(&mut self, e: &Event, cx: &mut Context<Self>) {
    match e {
      Event::BotsChanged | Event::Routines { .. } | Event::Skills | Event::ChatsChanged => self.load(cx),
      Event::Message { chat, .. } if chat == &self.chat => self.load(cx),
      _ => {}
    }
  }

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
          this.status = Some(e.to_string());
          cx.notify();
        }
      });
    })
    .detach();
  }

  pub fn bot_id(&self) -> Option<String> {
    self.data.bot.as_ref().map(|b| b.id.clone())
  }
}

pub fn section(title: impl Into<SharedString>, cx: &App) -> AnyElement {
  let ink = ink(cx);
  div().pt(px(14.0)).pb(px(6.0)).text_size(px(11.0)).font_weight(gpui::FontWeight::SEMIBOLD).text_color(ink.dimmed).child(title.into().to_uppercase()).into_any_element()
}

pub fn field(label: impl Into<SharedString>, control: impl IntoElement, cx: &App) -> AnyElement {
  let ink = ink(cx);
  div().flex().flex_col().gap(px(4.0)).pb(px(10.0)).child(div().text_size(px(12.0)).text_color(ink.dimmed).child(label.into())).child(control).into_any_element()
}

impl Render for Details {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let group = self.data.chat.as_ref().is_some_and(|c| c.is_group());
    let root = self.root.clone();
    let title = if group { t("Group details") } else { t("Details") };
    let mut tabs = div().flex().flex_wrap().gap(px(4.0)).px(px(16.0)).py(px(8.0)).border_b_1().border_color(ink.border);
    if !group {
      for (tab, label) in TABS {
        let on = self.tab == tab;
        tabs = tabs.child(
          div()
            .id(label)
            .px(px(10.0))
            .py(px(4.0))
            .rounded(px(6.0))
            .text_size(px(12.5))
            .cursor_pointer()
            .when(on, |d| d.bg(ink.hover).font_weight(gpui::FontWeight::SEMIBOLD))
            .hover(|s| s.bg(ink.hover))
            .on_click(cx.listener(move |this, _, _, cx| {
              this.tab = tab;
              this.routine = None;
              this.status = None;
              cx.notify();
            }))
            .child(t(label)),
        );
      }
    }
    let body = if group {
      group::render(self, window, cx)
    } else {
      match self.tab {
        Tab::Settings => profile::render(self, window, cx),
        Tab::Tasks => routines::render(self, window, cx),
        Tab::Secrets => secrets::render(self, window, cx),
        Tab::Memory => memory::render(self, cx),
        Tab::Skills => skills::render(self, cx),
        Tab::Share => share::render(self, window, cx),
      }
    };
    div()
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
          .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
          .child(ActionIcon::new("close-details", IconName::X).variant(Variant::Subtle).on_click(move |_, _, cx| {
            let _ = root.update(cx, |r, cx| r.close_right(cx));
          })),
      )
      .when(!group, |d| d.child(tabs))
      .child(div().id("details-body").flex_1().min_h_0().overflow_y_scroll().px(px(16.0)).pb(px(24.0)).child(div().w_full().child(body)))
      .when_some(self.status.clone(), |d, s| d.child(div().px(px(16.0)).py(px(8.0)).text_size(px(12.0)).text_color(ink.primary).border_t_1().border_color(ink.border).child(s)))
  }
}
