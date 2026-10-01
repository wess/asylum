//! Bot settings: avatar, name, label, description, notifications, and the
//! provider/model this Bot uses.

use super::{field, section, Details};
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString, Window};
use guise::{Button, Group, Size, Switch, Variant};
use store::bots::Profile;

pub const DESCRIPTION_LIMIT: usize = 140;

pub fn render(d: &mut Details, _window: &mut Window, cx: &mut Context<Details>) -> AnyElement {
  let ink = ink(cx);
  let Some(b) = d.data.bot.clone() else { return div().into_any_element() };
  let team = b.is_team();
  let desc_len = d.description.read(cx).text().chars().count();
  let profiles = d.rt.profiles();
  let mut col = div().flex().flex_col();
  col = col.child(
    div()
      .flex()
      .flex_col()
      .items_center()
      .gap(px(8.0))
      .py(px(16.0))
      .child(
        div()
          .id("avatar")
          .cursor_pointer()
          .child(crate::avatar::face(&b, 72.0, cx))
          .on_click(cx.listener(|this, _, w, cx| {
            let (root, id) = (this.root.clone(), this.bot_id());
            if let Some(id) = id {
              let _ = root.update(cx, |r, cx| crate::avatar::picker::open(r, &id, w, cx));
            }
          })),
      )
      .child(div().text_size(px(11.5)).text_color(ink.dimmed).child(t("Click to change avatar"))),
  );
  col = col
    .child(section(t("Bot settings"), cx))
    .child(field(t("Name"), d.name.clone(), cx))
    .child(field(t("Label (optional)"), d.label.clone(), cx))
    .child(field(if team { format!("{} ({desc_len}/{DESCRIPTION_LIMIT})", t("Description")) } else { t("Description").to_string() }, d.description.clone(), cx))
    .child(div().text_size(px(11.5)).text_color(ink.dimmed).pb(px(10.0)).child(t("The description holds lasting rules. Put task-specific instructions in messages.")));
  let bid = b.id.clone();
  col = col.child(
    Button::new("save-profile", t("Save")).size(Size::Sm).on_click(cx.listener(move |this, _, _, cx| {
      let p = Profile {
        name: this.name.read(cx).text().trim().to_string(),
        label: this.label.read(cx).text().trim().to_string(),
        description: this.description.read(cx).text(),
        avatar: this.data.bot.as_ref().map(|b| b.avatar.clone()).unwrap_or_default(),
        color: this.data.bot.as_ref().map(|b| b.color.clone()).unwrap_or_default(),
      };
      if this.data.bot.as_ref().is_some_and(|b| b.is_team()) && p.description.chars().count() > DESCRIPTION_LIMIT {
        this.status = Some(crate::i18n::tf("Keep a Team Bot's description to {} characters.", &[&DESCRIPTION_LIMIT.to_string()]));
        cx.notify();
        return;
      }
      let rt = this.rt.clone();
      let id = bid.clone();
      this.run(cx, async move { agent::api::bots::update(&rt, &id, &p).await }, |this, _, cx| {
        this.status = Some(t("Saved.").into());
        cx.notify();
      });
    })),
  );

  let (bid2, on) = (b.id.clone(), b.notifications);
  col = col.child(section(t("Notifications"), cx)).child(
    div()
      .flex()
      .items_center()
      .gap(px(10.0))
      .child(div().flex_1().min_w_0().text_size(px(13.0)).child(t("Get notified when this Bot finishes or needs input")))
      .child(Switch::new("bot-notif").checked(on).color(guise::ColorName::Violet).on_change(cx.listener(move |this, _, _, cx| {
        let rt = this.rt.clone();
        let id = bid2.clone();
        this.run(cx, async move { store::bots::set_notifications(&rt.pool, &id, !on).await }, |this, _, cx| this.load(cx));
      }))),
  );

  // Provider and model.
  let current = if b.provider.is_empty() { t("Default").to_string() } else { format!("{} · {}", b.provider, if b.model.is_empty() { t("default model") } else { &b.model }) };
  let mut chips = div().flex().flex_wrap().gap(px(4.0));
  let bid3 = b.id.clone();
  chips = chips.child(chip("model-default", t("Default").into(), b.provider.is_empty(), &ink).on_click(cx.listener(move |this, _, _, cx| {
    let rt = this.rt.clone();
    let id = bid3.clone();
    this.run(cx, async move { store::bots::set_model(&rt.pool, &id, "", "").await }, |this, _, cx| this.load(cx));
  })));
  for p in &profiles {
    for m in p.models.iter().take(8) {
      let on = b.provider == p.name && &b.model == m;
      let (bid, pn, mm) = (b.id.clone(), p.name.clone(), m.clone());
      chips = chips.child(chip(SharedString::from(format!("m-{}-{m}", p.name)), format!("{} · {m}", p.name).into(), on, &ink).on_click(cx.listener(move |this, _, _, cx| {
        let rt = this.rt.clone();
        let (id, pn, mm) = (bid.clone(), pn.clone(), mm.clone());
        this.run(cx, async move { store::bots::set_model(&rt.pool, &id, &pn, &mm).await }, |this, _, cx| this.load(cx));
      })));
    }
  }
  let bid4 = b.id.clone();
  col = col
    .child(section(t("Model"), cx))
    .child(div().text_size(px(13.0)).pb(px(6.0)).child(SharedString::from(current)))
    .child(chips)
    .child(div().pt(px(8.0)).text_size(px(11.5)).text_color(ink.dimmed).child(t("Or type provider/model:")))
    .child(
      Group::new()
        .gap(Size::Xs)
        .child(div().w(px(220.0)).child(d.model.clone()))
        .child(Button::new("set-model", t("Use")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| {
          let text = this.model.read(cx).text();
          let (p, m) = text.split_once('/').map(|(a, b)| (a.trim().to_string(), b.trim().to_string())).unwrap_or((text.trim().to_string(), String::new()));
          if !p.is_empty() && this.rt.profile(&p).is_none() {
            this.status = Some(crate::i18n::tf("No provider named {}.", &[&p]));
            cx.notify();
            return;
          }
          let rt = this.rt.clone();
          let id = bid4.clone();
          this.run(cx, async move { store::bots::set_model(&rt.pool, &id, &p, &m).await }, |this, _, cx| this.load(cx));
        }))),
    );

  // Actions.
  let (bid5, bid6, name) = (b.id.clone(), b.id.clone(), b.name.clone());
  col = col.child(section(t("Bot"), cx)).child(
    Group::new()
      .gap(Size::Xs)
      .wrap(true)
      .child(Button::new("dup", t("Duplicate")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(move |this, _, _, cx| {
        let rt = this.rt.clone();
        let id = bid5.clone();
        let root = this.root.clone();
        this.run(cx, async move { agent::api::bots::duplicate(&rt, &id).await }, move |_, (_, c), cx| {
          let _ = root.update(cx, |r, cx| {
            r.reload(cx);
            r.pending_open(c.id, cx);
          });
        });
      })))
      .child(Button::new("delete-bot", t("Delete")).size(Size::Xs).variant(Variant::Light).color(guise::ColorName::Red).on_click(cx.listener(move |this, _, w, cx| {
        let id = bid6.clone();
        let name = name.clone();
        let _ = this.root.update(cx, |r, cx| {
          crate::root::dialogs::confirm(
            r,
            crate::i18n::tf("Delete {}?", &[&name]),
            t("This removes the Bot, its conversation, and its routines. Files on the computer and browser sign-ins stay."),
            t("Delete"),
            w,
            cx,
            move |r, _, cx| {
              let rt = r.rt.clone();
              let id = id.clone();
              r.active = None;
              r.pane = None;
              r.close_right(cx);
              r.run(cx, async move { agent::api::bots::delete(&rt, &id).await }, |r, _, cx| r.reload(cx));
            },
          );
        });
      }))),
  );
  col.into_any_element()
}

pub fn chip(id: impl Into<gpui::ElementId>, label: SharedString, on: bool, ink: &crate::theme::Ink) -> gpui::Stateful<gpui::Div> {
  div()
    .id(id)
    .px(px(8.0))
    .py(px(2.0))
    .rounded_full()
    .text_size(px(11.5))
    .border_1()
    .border_color(if on { ink.primary } else { ink.border })
    .bg(if on { ink.primary.opacity(0.15) } else { ink.body })
    .cursor_pointer()
    .child(label)
}
