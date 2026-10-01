//! Previewing a Bot template or Team Bot link before adding it: what it
//! contains, warnings, and the third-party Bot terms to accept.

use crate::i18n::{t, tf};
use crate::root::Root;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, Context, SharedString, WeakEntity, Window};
use guise::{Button, Checkbox, Group, Size, Variant};

pub struct Preview {
  root: WeakEntity<Root>,
  link: String,
  tpl: Result<agent::template::Template, String>,
  agreed: bool,
}

pub fn preview(root: &mut Root, link: &str, _window: &mut Window, cx: &mut Context<Root>) {
  let weak = cx.entity().downgrade();
  let link = link.trim().to_string();
  let tpl = agent::template::parse(&link).map_err(|e| e.to_string());
  if tpl.is_err() {
    crate::root::problem::show(root, agent::api::team::NOT_FOUND, cx);
    return;
  }
  if tpl.as_ref().is_ok_and(|t| t.visibility == "team") && !root.rt.policy().team_bots_allowed() {
    crate::root::problem::show(root, agent::api::team::NOT_AVAILABLE, cx);
    return;
  }
  let view = cx.new(|_| Preview { root: weak, link, tpl, agreed: false });
  root.set_modal(view, cx);
}

impl Preview {
  fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let Ok(tpl) = self.tpl.clone() else { return };
    let link = self.link.clone();
    let _ = self.root.update(cx, |r, cx| {
      r.close_modal(window, cx);
      let rt = r.rt.clone();
      r.run(cx, async move {
        if tpl.visibility == "team" {
          agent::api::team::join(&rt, &link).await.map(|(_, c)| c)
        } else {
          let b = agent::template::install(&rt, &tpl).await?;
          store::chats::direct(&rt.pool, &b.id).await
        }
      }, |r, c, cx| {
        r.reload(cx);
        r.pending_open(c.id, cx);
      });
    });
  }
}

impl Render for Preview {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let root = self.root.clone();
    let body = match &self.tpl {
      Err(e) => div().text_color(ink.danger).child(SharedString::from(e.clone())).into_any_element(),
      Ok(tpl) => {
        let bot = store::Bot { name: tpl.name.clone(), avatar: tpl.avatar.clone(), color: tpl.color.clone(), ..Default::default() };
        let team = tpl.visibility == "team";
        let mut col = div()
          .flex()
          .flex_col()
          .gap(px(10.0))
          .child(div().flex().items_center().gap(px(12.0)).child(crate::avatar::face(&bot, 48.0, cx)).child(div().flex().flex_col().child(div().text_size(px(17.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(tpl.name.clone())).child(div().text_size(px(12.5)).text_color(ink.dimmed).child(if team { t("Team Bot") } else { t("Bot template") }))))
          .when(!tpl.label.is_empty(), |c| c.child(div().text_size(px(13.0)).child(tpl.label.clone())))
          .when(!tpl.description.is_empty(), |c| c.child(div().text_size(px(13.0)).text_color(ink.dimmed).child(tpl.description.clone())))
          .child(div().text_size(px(12.5)).child(tf("Includes {} skills and {} routines.", &[&tpl.skills.len().to_string(), &tpl.routines.len().to_string()])));
        for s in &tpl.skills {
          col = col.child(div().pl(px(10.0)).text_size(px(12.0)).text_color(ink.dimmed).child(format!("/{}", s.name)));
        }
        for r in &tpl.routines {
          col = col.child(div().pl(px(10.0)).text_size(px(12.0)).text_color(ink.dimmed).child(format!("{} — {}", r.name, schedule::describe(&r.trigger, &r.schedule, &r.filter.to_string()))));
        }
        col = col.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("Adding creates your own copy. The creator's computer, sign-ins, memory, and conversations are not included.")));
        for w in agent::template::warnings(tpl) {
          col = col.child(div().text_size(px(12.0)).text_color(ink.warning).child(SharedString::from(w)));
        }
        let agreed = self.agreed;
        col
          .child(
            div()
              .id("terms")
              .flex()
              .items_center()
              .gap(px(8.0))
              .cursor_pointer()
              .on_click(cx.listener(|p, _, _, cx| {
                p.agreed = !p.agreed;
                cx.notify();
              }))
              .child(Checkbox::new("terms-ck").checked(agreed))
              .child(div().text_size(px(12.5)).child(t("I accept the third-party Bot terms: this Bot was made by someone else, and I'm responsible for what I let it do."))),
          )
          .child(Group::new().gap(Size::Sm).child(Button::new("add-bot", t("Add to Asylum")).disabled(!agreed).on_click(cx.listener(|p, _, w, cx| p.add(w, cx)))).child(Button::new("cancel-bot", t("Cancel")).variant(Variant::Default).on_click(move |_, w, cx| {
            let _ = root.update(cx, |r, cx| r.close_modal(w, cx));
          })))
          .into_any_element()
      }
    };
    let root2 = self.root.clone();
    div().absolute().top_0().left_0().size_full().child(guise::Modal::new().width(480.0).on_close(move |_, w, cx| {
      let _ = root2.update(cx, |r, cx| r.close_modal(w, cx));
    }).child(body))
  }
}
