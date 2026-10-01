//! A group chat's name, description (read by every member Bot), and
//! members (2 to 6).

use super::{field, section, Details};
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString, Window};
use guise::{Button, Checkbox, Size};

pub fn render(d: &mut Details, _window: &mut Window, cx: &mut Context<Details>) -> AnyElement {
  let ink = ink(cx);
  let Some(c) = d.data.chat.clone() else { return div().into_any_element() };
  let cid = c.id.clone();
  let mut col = div()
    .flex()
    .flex_col()
    .pt(px(10.0))
    .child(field(t("Group name"), d.name.clone(), cx))
    .child(field(t("Description"), d.description.clone(), cx))
    .child(div().text_size(px(11.5)).text_color(ink.dimmed).pb(px(8.0)).child(t("Every member Bot reads the description.")))
    .child(Button::new("save-group", t("Save")).size(Size::Sm).on_click(cx.listener(move |this, _, _, cx| {
      let name = this.name.read(cx).text().trim().to_string();
      let desc = this.description.read(cx).text();
      let rt = this.rt.clone();
      let id = cid.clone();
      this.run(cx, async move {
        if !name.is_empty() {
          store::chats::rename(&rt.pool, &id, &name).await?;
        }
        store::chats::set_description(&rt.pool, &id, &desc).await?;
        rt.emit(agent::Event::ChatsChanged);
        Ok(())
      }, |this, _, cx| {
        this.status = Some(t("Saved.").into());
        cx.notify();
      });
    })))
    .child(section(t("Members"), cx));
  let current: Vec<String> = d.data.members.iter().map(|b| b.id.clone()).collect();
  for b in d.data.all.clone().into_iter().filter(|b| b.kind != "system") {
    let on = current.contains(&b.id);
    let (bid, cid) = (b.id.clone(), c.id.clone());
    let cur = current.clone();
    col = col.child(
      div()
        .id(SharedString::from(format!("gm-{}", b.id)))
        .flex()
        .items_center()
        .gap(px(8.0))
        .py(px(4.0))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
          let mut ids = cur.clone();
          if on {
            ids.retain(|x| x != &bid);
          } else {
            ids.push(bid.clone());
          }
          let rt = this.rt.clone();
          let cid = cid.clone();
          this.run(cx, async move { agent::api::bots::set_members(&rt, &cid, &ids).await }, |this, _, cx| this.load(cx));
        }))
        .child(Checkbox::new(SharedString::from(format!("gmc-{}", b.id))).checked(on))
        .child(crate::avatar::face(&b, 22.0, cx))
        .child(b.name.clone()),
    );
  }
  col.child(div().pt(px(8.0)).text_size(px(11.5)).text_color(ink.dimmed).child(if d.rt.settings().voice_enabled { t("Groups have 2 to 6 Bots. Voice chat isn't available in groups.") } else { t("Groups have 2 to 6 Bots.") })).into_any_element()
}
