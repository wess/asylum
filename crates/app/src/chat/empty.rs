use super::ChatPane;
use crate::i18n::t;
use crate::root::Root;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, Context, Window};
use guise::{Button, Size};

/// No conversation open.
pub fn render(root: &mut Root, _window: &mut Window, cx: &mut Context<Root>) -> impl IntoElement {
  let ink = ink(cx);
  div()
    .flex_1()
    .h_full()
    .flex()
    .flex_col()
    .items_center()
    .justify_center()
    .gap(px(14.0))
    .child(div().text_color(ink.primary).child(guise::Icon::new(guise::IconName::Bot).size(Size::Xl)))
    .child(div().text_size(px(20.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(t("Your team of Bots")))
    .child(div().text_color(ink.dimmed).max_w(px(420.0)).text_center().child(t("Bots are AI teammates with names, jobs, and their own computer. They keep working while you're away.")))
    .child(
      Button::new("empty-new", t("New Bot"))
        .left_section(guise::Icon::new(guise::IconName::Plus).size(Size::Sm))
        .on_click(cx.listener(|this, _, w, cx| crate::newchat::open(this, w, cx))),
    )
    .when(!root.snap.bots.is_empty(), |d| d.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("Or pick a Bot in the sidebar."))))
}

/// A conversation with no messages yet.
pub fn intro(pane: &ChatPane, cx: &mut Context<ChatPane>) -> impl IntoElement {
  let ink = ink(cx);
  let (title, body) = match pane.bot() {
    Some(b) if !pane.is_group() => (
      b.name.clone(),
      if b.label.is_empty() {
        t("Give this Bot a job. Tell it what it's responsible for, or open its details to set a label and standing instructions.").to_string()
      } else {
        crate::i18n::tf("{} is ready. Ask it to take something off your plate.", &[&b.label])
      },
    ),
    _ => (
      pane.chat.as_ref().map(|c| c.title.clone()).unwrap_or_default(),
      t("Everyone here can see this chat. Mention a Bot with @ to ask it directly, or @everyone.").to_string(),
    ),
  };
  let mut col = div().flex().flex_col().items_center().gap(px(10.0)).py(px(60.0));
  if let Some(b) = pane.bot().filter(|_| !pane.is_group()) {
    col = col.child(crate::avatar::face(b, 64.0, cx));
  }
  col
    .child(div().text_size(px(18.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(title))
    .child(div().max_w(px(460.0)).text_center().text_color(ink.dimmed).child(body))
}
