//! Usage: tokens your Bots used this week against an optional weekly limit,
//! and an optional monthly allowance for going past it. Nothing is charged;
//! the limits only cap your own provider spend.

use super::{heading, row, switch, Dialog};
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString};

pub fn tokens(n: u64) -> String {
  match n {
    n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
    n if n >= 1_000 => format!("{:.0}K", n as f64 / 1e3),
    n => n.to_string(),
  }
}

pub fn render(d: &mut Dialog, cx: &mut Context<Dialog>) -> AnyElement {
  let ink = ink(cx);
  let u = d.usage;
  let s = d.rt.settings();
  let pct = (u.fraction() * 100.0).round();
  let mut col = div()
    .flex()
    .flex_col()
    .child(heading(t("Weekly usage"), cx))
    .child(
      div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .py(px(8.0))
        .child(div().flex().justify_between().child(format!("{} / {} {}", tokens(u.week), tokens(u.weekly_limit), t("tokens"))).child(format!("{pct}%")))
        .child(guise::Progress::new(u.fraction() * 100.0).size(Size::Sm)),
    )
    .child(row(t("Weekly limit"), Some(t("Tokens per week before Agents pause. Resets every Monday.")), div().w(px(160.0)).child(d.weekly.clone()), cx))
    .child(heading(t("Past the weekly limit"), cx))
    .child(row(t("Keep going"), Some(t("Let Agents keep working past the weekly limit, up to the monthly allowance. A running Agent finishes its turn.")), switch("ondemand", s.on_demand, &d.rt, |s, v| s.on_demand = v), cx))
    .child(row(t("Monthly allowance past the limit"), Some(&format!("{} {}", tokens(u.month_over), t("used this month"))), div().w(px(160.0)).child(d.monthly.clone()), cx))
    .child(heading(t("This week by Agent"), cx));
  for (name, n) in d.by_bot.clone() {
    col = col.child(div().flex().justify_between().py(px(4.0)).text_size(px(13.0)).child(SharedString::from(name)).child(div().text_color(ink.dimmed).child(tokens(n.max(0) as u64))));
  }
  if d.by_bot.is_empty() {
    col = col.child(div().text_size(px(13.0)).text_color(ink.dimmed).child(t("No usage yet this week.")));
  }
  col.into_any_element()
}

use guise::Size;
