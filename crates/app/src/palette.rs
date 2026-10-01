//! ⌘K: jump to Bots and groups, search messages, files, and routines
//! across every conversation, and run common actions. ⇧⌘F scopes it to Bots.

use crate::i18n::t;
use crate::root::Root;
use crate::tk;
use agent::api::search::{Hit, Scope};
use gpui::prelude::*;
use gpui::{div, px, App, Context, Entity, SharedString, Subscription, WeakEntity, Window};
use guise::{IconName, SegmentedControl, SegmentedControlEvent, Size, TextInput, TextInputEvent};

pub const SCOPES: [(&str, Scope); 6] = [
  ("All", Scope::All),
  ("Messages", Scope::Messages),
  ("Agents", Scope::Bots),
  ("Group Chats", Scope::Groups),
  ("Files", Scope::Files),
  ("Routines", Scope::Routines),
];

pub struct Palette {
  root: WeakEntity<Root>,
  input: Entity<TextInput>,
  scopes: Entity<SegmentedControl>,
  scope: Scope,
  hits: Vec<Hit>,
  selected: usize,
  _subs: Vec<Subscription>,
}

pub fn open(root: &mut Root, scope: Option<Scope>, window: &mut Window, cx: &mut Context<Root>) {
  let weak = cx.entity().downgrade();
  let rt = root.rt.clone();
  let view = cx.new(|cx| {
    let input = cx.new(|cx| TextInput::new(cx).placeholder(t("Search Agents, messages, files, routines…")));
    let start = scope.unwrap_or(Scope::All);
    let idx = SCOPES.iter().position(|(_, s)| *s == start).unwrap_or(0);
    let scopes = cx.new(|cx| SegmentedControl::new(cx).data(SCOPES.iter().map(|(l, _)| t(l))).selected(idx).size(Size::Xs));
    window.focus(&input.read(cx).focus_handle(), cx);
    let rt2 = rt.clone();
    let s1 = cx.subscribe_in(&input, window, move |this: &mut Palette, _, ev: &TextInputEvent, w, cx| match ev {
      TextInputEvent::Change(q) => this.search(rt2.clone(), q.clone(), cx),
      TextInputEvent::Submit(_) => this.choose(this.selected, w, cx),
    });
    let rt3 = rt.clone();
    let s2 = cx.subscribe(&scopes, move |this: &mut Palette, _, ev: &SegmentedControlEvent, cx| {
      this.scope = SCOPES[ev.0].1;
      let q = this.input.read(cx).text();
      this.search(rt3.clone(), q, cx);
    });
    let mut p = Palette { root: weak, input, scopes, scope: start, hits: Vec::new(), selected: 0, _subs: vec![s1, s2] };
    p.search(rt.clone(), String::new(), cx);
    p
  });
  root.set_modal(view, cx);
}

impl Palette {
  fn search(&mut self, rt: agent::Runtime, q: String, cx: &mut Context<Self>) {
    let scope = self.scope;
    cx.spawn(async move |this, cx| {
      let hits = tk::run(async move { agent::api::search::search(&rt, &q, scope).await }).await.unwrap_or_default();
      let _ = this.update(cx, |this, cx| {
        this.hits = hits;
        this.selected = 0;
        cx.notify();
      });
    })
    .detach();
  }

  fn actions() -> Vec<(&'static str, Box<dyn gpui::Action>)> {
    vec![
      ("New Agent", Box::new(crate::actions::NewChat)),
      ("Open Settings", Box::new(crate::actions::OpenSettings)),
      ("Marketplace", Box::new(crate::actions::Marketplace)),
      ("Open Computer", Box::new(crate::actions::OpenComputer)),
      ("Conversation Details", Box::new(crate::actions::Details)),
      ("Toggle Sidebar", Box::new(crate::actions::ToggleSidebar)),
    ]
  }

  fn choose(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
    let q = self.input.read(cx).text().to_lowercase();
    let acts: Vec<_> = Self::actions().into_iter().filter(|(l, _)| self.scope == Scope::All && (q.is_empty() || t(l).to_lowercase().contains(&q))).collect();
    let root = self.root.clone();
    if i < self.hits.len() {
      let hit = self.hits[i].clone();
      let _ = root.update(cx, |r, cx| {
        r.close_modal(window, cx);
        match hit {
          Hit::Bot { id, .. } => r.open_item(&crate::state::Item::Bot(id), window, cx),
          Hit::Group { id, .. } => r.open(&id, window, cx),
          Hit::Message { chat, id, .. } => {
            r.open(&chat, window, cx);
            if let Some(p) = r.pane.clone() {
              p.update(cx, |p, cx| {
                p.focus = Some(id);
                p.load(cx);
              });
            }
          }
          Hit::Routine { bot, .. } => {
            r.open_item(&crate::state::Item::Bot(bot), window, cx);
            r.show_details(window, cx);
          }
          Hit::File { path, .. } => {
            let full = r.rt.computer.workspace().join(path.trim_start_matches("~/"));
            cx.open_url(&format!("file://{}", full.display()));
          }
        }
      });
    } else if let Some((_, a)) = acts.into_iter().nth(i - self.hits.len()) {
      let _ = root.update(cx, |r, cx| r.close_modal(window, cx));
      window.dispatch_action(a, cx);
    }
  }
}

fn hit_row(h: &Hit, cx: &App) -> (IconName, SharedString, SharedString) {
  let _ = cx;
  match h {
    Hit::Bot { name, label, .. } => (IconName::Bot, name.clone().into(), label.clone().into()),
    Hit::Group { title, .. } => (IconName::Users, title.clone().into(), t("Group chat").into()),
    Hit::Message { snippet, .. } => (IconName::MessageSquare, snippet.clone().into(), t("Message").into()),
    Hit::File { name, path } => (IconName::File, name.clone().into(), path.clone().into()),
    Hit::Routine { name, when, .. } => (IconName::CalendarClock, name.clone().into(), when.clone().into()),
  }
}

impl Render for Palette {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = crate::theme::ink(cx);
    let q = self.input.read(cx).text().to_lowercase();
    let root = self.root.clone();
    let mut list = div().id("palette-list").flex().flex_col().max_h(px(420.0)).overflow_y_scroll().gap(px(2.0));
    let mut n = 0;
    for h in self.hits.iter().take(40) {
      let (icon, title, sub) = hit_row(h, cx);
      let i = n;
      list = list.child(row(n == self.selected, icon, title, sub, &ink).id(SharedString::from(format!("hit{n}"))).on_click(cx.listener(move |this, _, w, cx| this.choose(i, w, cx))));
      n += 1;
    }
    if self.scope == Scope::All {
      for (l, _) in Self::actions().into_iter().filter(|(l, _)| q.is_empty() || t(l).to_lowercase().contains(&q)) {
        let i = n;
        list = list.child(row(n == self.selected, IconName::Command, t(l).into(), t("Action").into(), &ink).id(SharedString::from(format!("act{n}"))).on_click(cx.listener(move |this, _, w, cx| this.choose(i, w, cx))));
        n += 1;
      }
    }
    if n == 0 {
      list = list.child(div().p(px(16.0)).text_color(ink.dimmed).child(t("No results")));
    }
    div().absolute().top_0().left_0().size_full().child(
      guise::Modal::new()
        .width(620.0)
        .on_close(move |_, w, cx| {
          let _ = root.update(cx, |r, cx| r.close_modal(w, cx));
        })
        .child(div().flex().flex_col().gap(px(10.0)).child(self.input.clone()).child(self.scopes.clone()).child(list)),
    )
  }
}

fn row(selected: bool, icon: IconName, title: SharedString, sub: SharedString, ink: &crate::theme::Ink) -> gpui::Stateful<gpui::Div> {
  let hover = ink.hover;
  div()
    .id("row")
    .flex()
    .items_center()
    .gap(px(10.0))
    .px(px(10.0))
    .py(px(7.0))
    .rounded(px(8.0))
    .cursor_pointer()
    .when(selected, |d| d.bg(hover))
    .hover(move |s| s.bg(hover))
    .child(div().text_color(ink.dimmed).child(guise::Icon::new(icon).size(Size::Sm)))
    .child(div().flex_1().min_w_0().truncate().child(title))
    .child(div().max_w(px(220.0)).truncate().text_size(px(12.0)).text_color(ink.dimmed).child(sub))
}
