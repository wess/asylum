//! The account corner: who's signed in (your name and default provider),
//! this week's usage at a glance, and a menu with Settings, Providers,
//! About, updates, and Quit.

use crate::i18n::t;
use crate::root::Root;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, Context, SharedString};
use guise::{IconName, Size};

pub fn render(root: &mut Root, compact: bool, cx: &mut Context<Root>) -> impl IntoElement {
  let ink = ink(cx);
  let s = root.rt.settings();
  let name = if s.user_name.is_empty() { t("You").to_string() } else { s.user_name.clone() };
  let provider = root.rt.profile("").map(|p| if s.model.is_empty() { p.name } else { format!("{} · {}", p.name, s.model) }).unwrap_or_default();
  let usage = root.snap.usage;
  let frac = usage.fraction();
  let initial: SharedString = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default().into();
  div()
    .id("account")
    .mt(px(6.0))
    .flex()
    .items_center()
    .gap(px(10.0))
    .px(px(8.0))
    .py(px(8.0))
    .rounded(px(8.0))
    .cursor_pointer()
    .hover(|s| s.bg(ink.hover))
    .when(compact, |d| d.justify_center())
    .on_click(cx.listener(|this, ev: &gpui::ClickEvent, w, cx| menu(this, ev.position(), w, cx)))
    .child(div().size(px(28.0)).flex_none().rounded_full().bg(ink.hover).flex().items_center().justify_center().text_size(px(12.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(initial))
    .when(!compact, |d| {
      d.child(
        div()
          .flex_1()
          .min_w_0()
          .flex()
          .flex_col()
          .gap(px(3.0))
          .child(div().truncate().text_size(px(13.0)).child(name))
          .child(div().truncate().text_size(px(11.0)).text_color(ink.dimmed).child(provider))
          .child(div().h(px(3.0)).w_full().rounded_full().bg(ink.hover).child(div().h_full().rounded_full().bg(if frac > 0.9 { ink.warning } else { ink.primary }).w(gpui::relative(frac.max(0.01))))),
      )
    })
}

fn menu(root: &mut Root, at: gpui::Point<gpui::Pixels>, window: &mut gpui::Window, cx: &mut Context<Root>) {
  let me = cx.entity().downgrade();
  let u = root.snap.usage;
  let (m1, m2, m3, m4) = (me.clone(), me.clone(), me.clone(), me.clone());
  let menu = crate::menu::Menu::default()
    .section(format!("{} {} / {}", t("Weekly usage"), crate::settings::usage::tokens(u.week), crate::settings::usage::tokens(u.weekly_limit)))
    .item(t("Settings"), move |w, cx| {
      let _ = m1.update(cx, |r, cx| crate::settings::open(r, None, w, cx));
    })
    .item(t("Providers"), move |w, cx| {
      let _ = m2.update(cx, |r, cx| crate::settings::open(r, Some("providers"), w, cx));
    })
    .item(t("Usage"), move |w, cx| {
      let _ = m3.update(cx, |r, cx| crate::settings::open(r, Some("usage"), w, cx));
    })
    .item(t("Check for Updates"), move |w, cx| {
      let _ = m4.update(cx, |r, cx| crate::settings::open(r, Some("updates"), w, cx));
    })
    .item(t("About Asylum"), |w, cx| w.dispatch_action(Box::new(crate::actions::About), cx))
    .divider()
    .item(t("Quit Asylum"), |_, cx| cx.quit());
  let _ = IconName::User;
  let _ = Size::Xs;
  root.menu = menu.show(at, 240.0, window, cx);
  cx.notify();
}
