//! Full-screen problems instead of toasts: Team Bots Not Available, Bot Not
//! Found, and the usage limit. `known` maps an engine error to one.

use super::Root;
use crate::i18n::t;
use crate::theme::ink;
use gpui::{div, prelude::*, px, Context, SharedString, Window};
use guise::{Button, Icon, IconName, Size};

pub struct Problem {
  root: gpui::WeakEntity<Root>,
  title: &'static str,
  body: &'static str,
  settings: Option<&'static str>,
}

/// The screen for an engine error, if it has one.
pub fn known(err: &str) -> Option<(&'static str, &'static str, Option<&'static str>)> {
  if err.contains(agent::api::team::NOT_AVAILABLE) {
    return Some((agent::api::team::NOT_AVAILABLE, "Your admin has turned off Team Bots for your organization. Ask them to turn it on.", None));
  }
  if err.contains(agent::api::team::NOT_FOUND) {
    return Some((agent::api::team::NOT_FOUND, "This link doesn't point to a Bot you can add. It may have been unpublished or the link is incomplete. Ask its owner for a new link.", None));
  }
  if err.contains("reached your usage limit") {
    return Some(("Usage limit reached", "Your Bots will pick up again when the week resets. To keep going now, raise your weekly limit or turn on Keep going in Settings → Usage.", Some("usage")));
  }
  None
}

pub fn show(root: &mut Root, err: &str, cx: &mut Context<Root>) -> bool {
  let Some((title, body, settings)) = known(err) else { return false };
  let weak = cx.entity().downgrade();
  let view = cx.new(|_| Problem { root: weak, title, body, settings });
  root.set_modal(view, cx);
  true
}

impl Render for Problem {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let (r1, r2) = (self.root.clone(), self.root.clone());
    let page = self.settings;
    let mut actions = div().flex().gap(px(8.0)).justify_center().child(Button::new("problem-ok", t("OK")).size(Size::Sm).on_click(move |_, w, cx| {
      let _ = r1.update(cx, |r, cx| r.close_modal(w, cx));
    }));
    if let Some(page) = page {
      actions = actions.child(Button::new("problem-settings", t("Open Settings")).size(Size::Sm).variant(guise::Variant::Light).on_click(move |_, w, cx| {
        let _ = r2.update(cx, |r, cx| {
          r.close_modal(w, cx);
          crate::settings::open(r, Some(page), w, cx);
        });
      }));
    }
    div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().bg(gpui::black().opacity(0.45)).child(
      div()
        .w(px(420.0))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(12.0))
        .p(px(28.0))
        .rounded(px(14.0))
        .bg(ink.body)
        .border_1()
        .border_color(ink.border)
        .shadow_lg()
        .child(div().text_color(ink.warning).child(Icon::new(IconName::CircleAlert).size(Size::Xl)))
        .child(div().text_size(px(17.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(t(self.title)))
        .child(div().text_size(px(13.0)).text_color(ink.dimmed).text_center().child(SharedString::from(t(self.body))))
        .child(actions),
    )
  }
}
