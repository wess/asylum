//! Update required: when a newer release exists and this build is more
//! than 14 days old, updating is required before continuing.

use crate::i18n::t;
use crate::root::Root;
use gpui::prelude::*;
use gpui::{div, px, Context, SharedString, WeakEntity, Window};
use guise::{Button, Group, Modal, Size, Variant};

pub const MAX_AGE_DAYS: i64 = 14;

pub fn age_days(built: &str, today: chrono::NaiveDate) -> i64 {
  chrono::NaiveDate::parse_from_str(built, "%Y-%m-%d").map(|d| (today - d).num_days()).unwrap_or(0)
}

pub fn required(latest: &str, current: &str, built: &str, today: chrono::NaiveDate) -> bool {
  super::updates::newer(latest, current) && age_days(built, today) > MAX_AGE_DAYS
}

pub struct Gate {
  root: WeakEntity<Root>,
  latest: String,
}

pub fn check(root: &mut Root, cx: &mut Context<Root>) {
  if !root.rt.settings().automatic_updates {
    return;
  }
  cx.spawn(async move |this, cx| {
    let Ok(Some(latest)) = crate::tk::run(super::updates::latest()).await else { return };
    let today = chrono::Local::now().date_naive();
    if !required(&latest, super::updates::VERSION, env!("ASYLUM_BUILD_DATE"), today) {
      return;
    }
    let _ = this.update(cx, |r, cx| {
      let weak = cx.entity().downgrade();
      let view = cx.new(|_| Gate { root: weak, latest });
      r.set_modal(view, cx);
    });
  })
  .detach();
}

impl Render for Gate {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = crate::theme::ink(cx);
    let root = self.root.clone();
    let url = format!("https://github.com/{}/releases/latest", super::updates::REPO);
    let url2 = url.clone();
    div().absolute().top_0().left_0().size_full().child(
      Modal::new().title(t("Update required")).width(440.0).child(
        div()
          .flex()
          .flex_col()
          .gap(px(10.0))
          .child(div().text_size(px(13.0)).child(SharedString::from(crate::i18n::tf("This version is more than 14 days old and version {} is available. Update to keep using Asylum.", &[&self.latest]))))
          .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(SharedString::from(crate::i18n::tf("Installed: {}", &[super::updates::VERSION]))))
          .child(
            Group::new()
              .gap(Size::Sm)
              .child(Button::new("update", t("Update")).on_click(move |_, _, cx| cx.open_url(&url)))
              .child(Button::new("try-again", t("Try again")).variant(Variant::Light).on_click(move |_, w, cx| {
                let _ = root.update(cx, |r, cx| {
                  r.close_modal(w, cx);
                  check(r, cx);
                });
              }))
              .child(Button::new("download", t("Download the latest version")).variant(Variant::Default).on_click(move |_, _, cx| cx.open_url(&url2))),
          ),
      ),
    )
  }
}

#[cfg(test)]
#[path = "../../tests/required.rs"]
mod tests;
