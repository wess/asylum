//! One conversation: header, transcript of messages and cards, error
//! notices, the composer, find-in-chat, and a thread pane.

pub mod empty;
pub mod find;
pub mod header;
pub mod message;
pub mod notices;
pub mod thread;

use crate::composer::Composer;
use crate::root::Root;
use crate::theme::ink;
use crate::tk;
use agent::{Event, Runtime};
use gpui::prelude::*;
use gpui::{div, px, Context, Entity, ScrollHandle, Subscription, WeakEntity, Window};
use std::collections::{HashMap, HashSet};
use store::{Approval, Bot, Chat, Message};

pub struct ChatPane {
  pub rt: Runtime,
  pub id: String,
  pub root: WeakEntity<Root>,
  pub chat: Option<Chat>,
  pub messages: Vec<Message>,
  pub bots: HashMap<String, Bot>,
  pub members: Vec<String>,
  pub approvals: HashMap<String, Approval>,
  pub threads: HashMap<String, i64>,
  pub streams: HashMap<String, String>,
  pub scroll: ScrollHandle,
  pub composer: Entity<Composer>,
  pub notices: Vec<(String, String)>,
  pub find: Option<find::Find>,
  pub thread: Option<Entity<thread::Thread>>,
  pub open: HashSet<String>,
  pub cards: crate::cards::Inputs,
  pub loaded: bool,
  /// A message to scroll to and highlight (from search).
  pub focus: Option<String>,
  /// Link hover previews: None while loading (or when unavailable).
  pub previews: HashMap<String, Option<computer::web::Preview>>,
  pub _subs: Vec<Subscription>,
}

impl ChatPane {
  pub fn new(rt: Runtime, id: String, root: WeakEntity<Root>, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let me = cx.entity().downgrade();
    let composer = cx.new(|cx| Composer::new(rt.clone(), id.clone(), None, me, window, cx));
    let mut pane = Self {
      rt,
      id,
      root,
      chat: None,
      messages: Vec::new(),
      bots: HashMap::new(),
      members: Vec::new(),
      approvals: HashMap::new(),
      threads: HashMap::new(),
      streams: HashMap::new(),
      scroll: ScrollHandle::new(),
      composer,
      notices: Vec::new(),
      find: None,
      thread: None,
      open: HashSet::new(),
      cards: Default::default(),
      loaded: false,
      focus: None,
      previews: HashMap::new(),
      _subs: Vec::new(),
    };
    pane.load(cx);
    pane.focus_composer(window, cx);
    pane
  }

  pub fn load(&mut self, cx: &mut Context<Self>) {
    let rt = self.rt.clone();
    let id = self.id.clone();
    cx.spawn(async move |this, cx| {
      let data = tk::run(async move {
        let pool = &rt.pool;
        let chat = store::chats::get(pool, &id).await?;
        let messages = store::messages::list(pool, &id).await?;
        let bots: HashMap<String, Bot> = store::bots::list(pool).await?.into_iter().map(|b| (b.id.clone(), b)).collect();
        let members = store::chats::members(pool, &id).await?;
        let mut approvals = HashMap::new();
        for m in &messages {
          for p in agent::part::parse(&m.parts) {
            if let agent::Part::Approval { id } = p {
              if let Ok(a) = store::approvals::get(pool, &id).await {
                approvals.insert(id, a);
              }
            }
          }
        }
        let threads = store::messages::thread_counts(pool, &id).await?.into_iter().collect();
        anyhow::Ok((chat, messages, bots, members, approvals, threads))
      })
      .await;
      let _ = this.update(cx, |this, cx| {
        if let Ok((chat, messages, bots, members, approvals, threads)) = data {
          let grew = messages.len() != this.messages.len();
          let at_bottom = this.at_bottom();
          this.chat = Some(chat);
          this.messages = messages;
          this.bots = bots;
          this.members = members;
          this.approvals = approvals;
          this.threads = threads;
          this.streams.retain(|id, _| this.messages.iter().any(|m| &m.id == id && m.status == "streaming"));
          if let Some(f) = this.focus.clone() {
            if let Some(i) = this.messages.iter().position(|m| m.id == f) {
              this.scroll.scroll_to_item(i + 1);
              // Let the highlight linger, then return to following new messages.
              cx.spawn(async move |this, cx| {
                cx.background_executor().timer(std::time::Duration::from_millis(2500)).await;
                let _ = this.update(cx, |p, cx| {
                  p.focus = None;
                  cx.notify();
                });
              })
              .detach();
            }
          } else if !this.loaded || (grew && at_bottom) {
            this.scroll.scroll_to_bottom();
          }
          this.loaded = true;
        }
        cx.notify();
      });
    })
    .detach();
  }

  fn at_bottom(&self) -> bool {
    let max = self.scroll.max_offset().y;
    let off = -self.scroll.offset().y;
    max - off < px(80.0)
  }

  pub fn on_event(&mut self, e: &Event, _window: &mut Window, cx: &mut Context<Self>) {
    match e {
      Event::Message { chat, .. } if chat == &self.id => self.load(cx),
      Event::Stream { chat, message, text } if chat == &self.id => {
        let follow = self.at_bottom();
        self.streams.insert(message.clone(), text.clone());
        if follow {
          self.scroll.scroll_to_bottom();
        }
        cx.notify();
      }
      Event::Notice { chat: Some(chat), text, request } if chat == &self.id => {
        self.notices.push((text.clone(), request.clone()));
        cx.notify();
      }
      Event::Approval { .. } | Event::BotsChanged | Event::BotStatus { .. } => self.load(cx),
      _ => {}
    }
    if let Some(t) = self.thread.clone() {
      t.update(cx, |t, cx| t.on_event(e, cx));
    }
  }

  pub fn is_group(&self) -> bool {
    self.chat.as_ref().is_some_and(|c| c.is_group())
  }

  pub fn bot(&self) -> Option<&Bot> {
    self.chat.as_ref().and_then(|c| c.bot_id.as_ref()).and_then(|b| self.bots.get(b))
  }

  pub fn busy(&self) -> bool {
    self.members.iter().any(|m| self.rt.queues.busy(m))
  }

  pub fn focus_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.composer.update(cx, |c, cx| c.focus(window, cx));
  }

  pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.composer.update(cx, |c, cx| c.submit(window, cx));
  }

  pub fn dictate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.composer.update(cx, |c, cx| c.dictate(window, cx));
  }

  pub fn toggle_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.find.is_some() {
      self.find = None;
    } else {
      self.find = Some(find::Find::new(window, cx));
    }
    cx.notify();
  }

  pub fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.find.is_some() {
      self.find = None;
    } else if self.thread.is_some() {
      self.thread = None;
    } else {
      self.composer.update(cx, |c, cx| c.escape(window, cx));
    }
    cx.notify();
  }

  /// Leaving the chat saves the unsent draft.
  pub fn leave(&mut self, cx: &mut Context<Self>) {
    let draft = self.composer.read(cx).text(cx);
    let rt = self.rt.clone();
    let id = self.id.clone();
    tk::spawn(async move { store::chats::save_draft(&rt.pool, &id, &draft).await });
  }

  pub fn open_thread(&mut self, root_msg: &str, window: &mut Window, cx: &mut Context<Self>) {
    let rt = self.rt.clone();
    let me = cx.entity().downgrade();
    let (chat, id) = (self.id.clone(), root_msg.to_string());
    self.thread = Some(cx.new(|cx| thread::Thread::new(rt, chat, id, me, window, cx)));
    cx.notify();
  }

  pub fn with_root(&self, cx: &mut Context<Self>, f: impl FnOnce(&mut Root, &mut Context<Root>)) {
    let _ = self.root.update(cx, f);
  }

  pub fn toast(&self, msg: impl Into<String>, cx: &mut Context<Self>) {
    let msg = msg.into();
    self.with_root(cx, |r, cx| r.toast(msg, cx));
  }

  /// Run an engine call; errors become toasts.
  pub fn run<F, T>(&mut self, cx: &mut Context<Self>, f: F, then: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static)
  where
    F: std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
    T: Send + 'static,
  {
    cx.spawn(async move |this, cx| {
      let r = tk::run(f).await;
      let _ = this.update(cx, |this, cx| match r {
        Ok(v) => then(this, v, cx),
        Err(e) => this.toast(e.to_string(), cx),
      });
    })
    .detach();
  }
}

impl Render for ChatPane {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let query = self.find.as_ref().map(|f| f.query(cx)).unwrap_or_default();
    // Each message is a direct child of the scroller so search can scroll
    // to it; the first child is the top padding / intro.
    let row = || div().w_full().min_w_0().max_w(px(880.0)).mx_auto().px(px(28.0)).pt(px(14.0));
    let mut transcript = div()
      .id("transcript")
      .flex_1()
      .min_h_0()
      .w_full()
      .min_w_0()
      .pb(px(20.0))
      .overflow_y_scroll()
      .track_scroll(&self.scroll);
    transcript = transcript.child(if self.loaded && self.messages.is_empty() { row().child(empty::intro(self, cx)).into_any_element() } else { div().h(px(6.0)).into_any_element() });
    let n = self.messages.len();
    for i in 0..n {
      let m = self.messages[i].clone();
      if !query.is_empty() && !m.body.to_lowercase().contains(&query.to_lowercase()) {
        continue;
      }
      let hot = self.focus.as_deref() == Some(m.id.as_str());
      transcript = transcript.child(row().when(hot, |d| d.bg(ink.primary.opacity(0.08)).rounded(px(10.0))).child(message::render(self, &m, &query, window, cx)));
    }
    let mut col = div()
      .size_full()
      .flex()
      .flex_col()
      .bg(ink.body)
      .child(header::render(self, window, cx));
    if let Some(f) = &self.find {
      col = col.child(f.render(self, cx));
    }
    col = col
      .child(transcript)
      .child(notices::render(self, cx))
      .child(div().max_w(px(880.0)).w_full().mx_auto().px(px(24.0)).pb(px(18.0)).child(self.composer.clone()));
    let mut out = div().size_full().flex().child(div().flex_1().min_w_0().h_full().overflow_hidden().child(col));
    if let Some(t) = &self.thread {
      out = out.child(div().w(px(380.0)).h_full().flex_none().border_l_1().border_color(ink.border).child(t.clone()));
    }
    out
  }
}
