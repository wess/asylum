//! The custom title bar: room for the traffic lights, back/forward, the
//! open conversation's name, and the computer status icon (violet while any
//! Bot is working on the computer).

use super::Root;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, Context, SharedString, Window};
use guise::{ActionIcon, IconName, Size, Variant};

pub const HEIGHT: f32 = 44.0;

pub fn render(root: &mut Root, _window: &mut Window, cx: &mut Context<Root>) -> impl IntoElement {
  let ink = ink(cx);
  let busy = root.rt.queues.busy_any();
  let title: SharedString = match root.active_item() {
    Some(crate::state::Item::Bot(b)) => root.snap.bot(&b).map(|b| b.name.clone()).unwrap_or_default().into(),
    Some(crate::state::Item::Group(g)) => root.snap.groups.iter().find(|c| c.id == g).map(|c| c.title.clone()).unwrap_or_default().into(),
    None => "Asylum".into(),
  };
  let status_color = if busy { ink.primary } else { ink.dimmed };
  let state_label = match root.computer.as_str() {
    "starting" => t("Starting your computer"),
    "updating" => t("Updating your computer"),
    "recovering" | "resetting" => t("Reconnecting"),
    "recreating" => t("Recreating your computer"),
    "hibernating" => t("Computer hibernating"),
    "unreachable" => t("Couldn't reach the computer"),
    _ if busy => t("Computer active"),
    _ => t("Computer idle"),
  };
  div()
    .h(px(HEIGHT))
    .flex_none()
    .flex()
    .items_center()
    .border_b_1()
    .border_color(ink.border)
    .bg(ink.sidebar)
    .child(
      div()
        .id("drag")
        .flex_1()
        .h_full()
        .pl(px(84.0))
        .flex()
        .items_center()
        .gap(px(4.0))
        .window_control_area(gpui::WindowControlArea::Drag)
        .on_mouse_down(gpui::MouseButton::Left, |ev, window, _| {
          if ev.click_count == 2 {
            window.zoom_window();
          } else {
            window.start_window_move();
          }
        })
        .child(
          ActionIcon::new("sidebar", IconName::PanelLeft)
            .variant(Variant::Subtle)
            .size(Size::Sm)
            .on_click(cx.listener(|_, _, w, cx| w.dispatch_action(Box::new(crate::actions::ToggleSidebar), cx))),
        )
        .child(
          ActionIcon::new("back", IconName::ChevronLeft)
            .variant(Variant::Subtle)
            .size(Size::Sm)
            .on_click(cx.listener(|_, _, w, cx| w.dispatch_action(Box::new(crate::actions::Back), cx))),
        )
        .child(
          ActionIcon::new("fwd", IconName::ChevronRight)
            .variant(Variant::Subtle)
            .size(Size::Sm)
            .on_click(cx.listener(|_, _, w, cx| w.dispatch_action(Box::new(crate::actions::Forward), cx))),
        )
        .child(div().flex_1().flex().justify_center().text_size(px(13.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(title)),
    )
    .child(
      div()
        .id("computer-status")
        .px(px(12.0))
        .h_full()
        .flex()
        .items_center()
        .gap(px(6.0))
        .cursor_pointer()
        .tooltip(guise::tooltip(state_label))
        .on_click(cx.listener(|_, _, w, cx| w.dispatch_action(Box::new(crate::actions::OpenComputer), cx)))
        .child(div().text_color(status_color).child(guise::Icon::new(IconName::Monitor).size(Size::Sm)))
        .when(root.disk != "ok", |d| {
          d.child(div().size(px(8.0)).rounded_full().bg(if root.disk == "critical" { ink.danger } else { ink.warning }))
        }),
    )
}
