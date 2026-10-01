//! In-chat forms, one per step (a login, a shipping address, a phone number).

use super::{key, shell};
use crate::chat::ChatPane;
use crate::i18n::t;
use crate::theme::ink;
use agent::Part;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString, Window};
use guise::{Badge, Button, Size};
use store::Message;

pub fn render(pane: &mut ChatPane, m: &Message, i: usize, p: &Part, _window: &mut Window, cx: &mut Context<ChatPane>) -> AnyElement {
  let Part::Form { title, fields, status, values } = p else { return div().into_any_element() };
  let ink = ink(cx);
  let mut card = shell(cx).child(
    div()
      .flex()
      .items_center()
      .justify_between()
      .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(SharedString::from(title.clone())))
      .when(status != "pending", |d| d.child(Badge::new(t("Submitted")).size(Size::Xs).color(guise::ColorName::Green))),
  );
  if status != "pending" {
    for f in fields {
      let v = values[&f.name].as_str().unwrap_or("").to_string();
      card = card.child(div().flex().gap(px(10.0)).text_size(px(13.0)).child(div().w(px(140.0)).text_color(ink.dimmed).child(f.label.clone())).child(div().child(v)));
    }
    return card.into_any_element();
  }
  let mut keys = Vec::new();
  for f in fields {
    let k = key(m, i, &f.name);
    keys.push((f.name.clone(), k.clone(), f.required, f.label.clone()));
    let label = if f.required { format!("{} *", f.label) } else { f.label.clone() };
    let control: AnyElement = if f.kind == "long" {
      pane.cards.area(k, "", 3, cx).into_any_element()
    } else {
      pane.cards.line(k, "", false, cx).into_any_element()
    };
    card = card.child(div().flex().flex_col().gap(px(4.0)).child(div().text_size(px(12.0)).text_color(ink.dimmed).child(label)).child(control));
  }
  let mid = m.id.clone();
  card
    .child(Button::new(SharedString::from(format!("submit-{}", m.id)), t("Submit")).size(Size::Xs).on_click(cx.listener(move |this, _, _, cx| {
      let mut values = serde_json::Map::new();
      for (name, k, required, label) in &keys {
        let v = this.cards.text(k, cx);
        if *required && v.trim().is_empty() {
          this.toast(crate::i18n::tf("{} is required", &[label]), cx);
          return;
        }
        values.insert(name.clone(), serde_json::Value::String(v));
      }
      let rt = this.rt.clone();
      let mid = mid.clone();
      this.run(cx, async move { agent::api::cards::submit_form(&rt, &mid, i, serde_json::Value::Object(values)).await }, |_, _, _| {});
    })))
    .into_any_element()
}
