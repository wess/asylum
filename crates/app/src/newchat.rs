//! ⌘N: create a Bot (named "New Agent", or the typed name), create a Team
//! Bot, pick 2 to 6 Bots for a group chat, or browse Team Bots.

use crate::i18n::{t, tf};
use crate::root::Root;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, Context, Entity, SharedString, Subscription, WeakEntity, Window};
use guise::{Button, Checkbox, IconName, Size, TextInput, TextInputEvent, Variant};
use std::collections::BTreeSet;
use store::Bot;

pub struct NewChat {
  root: WeakEntity<Root>,
  input: Entity<TextInput>,
  bots: Vec<Bot>,
  picked: BTreeSet<String>,
  link: Entity<TextInput>,
  _subs: Vec<Subscription>,
}

pub fn open(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
  let weak = cx.entity().downgrade();
  let bots: Vec<Bot> = root.snap.bots.iter().filter(|b| b.kind != "system").cloned().collect();
  let view = cx.new(|cx| {
    let input = cx.new(|cx| TextInput::new(cx).placeholder(t("Search or name a new Agent")));
    let link = cx.new(|cx| TextInput::new(cx).placeholder(t("Paste a Team Agent or template link")).size(Size::Xs));
    window.focus(&input.read(cx).focus_handle(), cx);
    let s1 = cx.observe(&input, |_, _, cx| cx.notify());
    let s2 = cx.subscribe_in(&input, window, |this: &mut NewChat, _, ev: &TextInputEvent, w, cx| {
      if let TextInputEvent::Submit(name) = ev {
        let name = name.clone();
        this.create(Some(name), w, cx);
      }
    });
    NewChat { root: weak, input, bots, picked: BTreeSet::new(), link, _subs: vec![s1, s2] }
  });
  root.set_modal(view, cx);
}

impl NewChat {
  fn create(&mut self, name: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
    let name = name.filter(|n| !n.trim().is_empty() && !self.bots.iter().any(|b| b.name.eq_ignore_ascii_case(n.trim())));
    let _ = self.root.update(cx, |r, cx| {
      r.close_modal(window, cx);
      let rt = r.rt.clone();
      r.run(cx, async move { agent::api::bots::create(&rt, name.as_deref()).await }, |r, (_, c), cx| {
        r.reload(cx);
        r.pending_open(c.id, cx);
      });
    });
  }

  fn team(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let _ = self.root.update(cx, |r, cx| {
      r.close_modal(window, cx);
      let rt = r.rt.clone();
      r.run(cx, async move { agent::api::team::create(&rt).await }, |r, (_, c), cx| {
        r.reload(cx);
        r.pending_open(c.id, cx);
      });
    });
  }

  fn group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let ids: Vec<String> = self.picked.iter().cloned().collect();
    let _ = self.root.update(cx, |r, cx| {
      r.close_modal(window, cx);
      let rt = r.rt.clone();
      r.run(cx, async move { agent::api::bots::create_group(&rt, &ids).await }, |r, c, cx| {
        r.reload(cx);
        r.pending_open(c.id, cx);
      });
    });
  }

  fn join(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let link = self.link.read(cx).text();
    let _ = self.root.update(cx, |r, cx| {
      r.close_modal(window, cx);
      crate::template::preview(r, &link, window, cx);
    });
  }
}

impl Render for NewChat {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let q = self.input.read(cx).text();
    let ql = q.trim().to_lowercase();
    let root = self.root.clone();
    let exact = self.bots.iter().any(|b| b.name.to_lowercase() == ql);
    let mut col = div().flex().flex_col().gap(px(6.0)).child(self.input.clone());
    let label = if q.trim().is_empty() || exact { t("Create new Agent").to_string() } else { tf("Create \"{}\" Agent", &[q.trim()]) };
    let name = q.trim().to_string();
    col = col
      .child(action("create", IconName::Plus, label.into(), &ink).on_click(cx.listener(move |this, _, w, cx| this.create(Some(name.clone()), w, cx))))
      .child(action("team", IconName::Users, t("Create new Team Agent").into(), &ink).on_click(cx.listener(|this, _, w, cx| this.team(w, cx))));
    col = col.child(div().pt(px(8.0)).text_size(px(11.0)).text_color(ink.dimmed).child(t("START A GROUP CHAT WITH 2 TO 6 AGENTS")));
    let mut list = div().id("newchat-bots").flex().flex_col().max_h(px(280.0)).overflow_y_scroll();
    for b in self.bots.iter().filter(|b| ql.is_empty() || b.name.to_lowercase().contains(&ql) || b.label.to_lowercase().contains(&ql)) {
      let id = b.id.clone();
      let checked = self.picked.contains(&b.id);
      let id2 = id.clone();
      list = list.child(
        div()
          .id(SharedString::from(format!("nc-{id}")))
          .flex()
          .items_center()
          .gap(px(10.0))
          .px(px(8.0))
          .py(px(6.0))
          .rounded(px(8.0))
          .cursor_pointer()
          .hover(|s| s.bg(ink.hover))
          .on_click(cx.listener(move |this, _, _, cx| {
            if !this.picked.remove(&id) && this.picked.len() < 6 {
              this.picked.insert(id.clone());
            }
            cx.notify();
          }))
          .child(Checkbox::new(SharedString::from(format!("ck-{id2}"))).checked(checked))
          .child(crate::avatar::face(b, 24.0, cx))
          .child(div().flex_1().child(b.name.clone()))
          .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(b.label.clone())),
      );
    }
    col = col.child(list);
    let n = self.picked.len();
    col = col.child(
      Button::new("start-group", tf("Start group chat ({})", &[&n.to_string()]))
        .disabled(!(2..=6).contains(&n))
        .full_width(true)
        .on_click(cx.listener(|this, _, w, cx| this.group(w, cx))),
    );
    let teams: Vec<&Bot> = self.bots.iter().filter(|b| b.is_team() && b.published).collect();
    col = col.child(div().pt(px(8.0)).text_size(px(11.0)).text_color(ink.dimmed).child(t("TEAM AGENTS")));
    if ql.chars().count() >= 3 || ql.is_empty() {
      for b in teams.iter().filter(|b| ql.is_empty() || b.name.to_lowercase().contains(&ql)) {
        col = col.child(div().flex().gap(px(8.0)).items_center().px(px(8.0)).child(crate::avatar::face(b, 20.0, cx)).child(b.name.clone()).child(div().text_size(px(11.0)).text_color(ink.dimmed).child(b.owner.clone())));
      }
    } else {
      col = col.child(div().px(px(8.0)).text_size(px(12.0)).text_color(ink.dimmed).child(t("Search Team Agents needs at least 3 characters.")));
    }
    col = col.child(div().flex().gap(px(6.0)).child(div().flex_1().child(self.link.clone())).child(Button::new("add-link", t("Add")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, w, cx| this.join(w, cx)))));
    div().absolute().top_0().left_0().size_full().child(
      guise::Modal::new()
        .title(t("New chat"))
        .width(520.0)
        .on_close(move |_, w, cx| {
          let _ = root.update(cx, |r, cx| r.close_modal(w, cx));
        })
        .child(col),
    )
  }
}

fn action(id: &'static str, icon: IconName, label: SharedString, ink: &crate::theme::Ink) -> gpui::Stateful<gpui::Div> {
  let hover = ink.hover;
  div()
    .id(id)
    .flex()
    .items_center()
    .gap(px(10.0))
    .px(px(8.0))
    .py(px(8.0))
    .rounded(px(8.0))
    .cursor_pointer()
    .hover(move |s| s.bg(hover))
    .child(div().size(px(24.0)).rounded_full().bg(ink.primary.opacity(0.15)).text_color(ink.primary).flex().items_center().justify_center().child(guise::Icon::new(icon).size(Size::Xs)))
    .child(label)
}
