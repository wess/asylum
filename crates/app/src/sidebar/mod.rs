//! The sidebar: New, Search, and Marketplace; pinned rows; rows grouped by
//! section; Hidden Bots at the bottom. Compact mode shows avatars only.

pub mod menu;
pub mod row;

use crate::i18n::t;
use crate::root::Root;
use crate::state::Item;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, Context, SharedString, Window};
use guise::{IconName, Size};

pub const WIDTH: f32 = 264.0;
pub const COMPACT: f32 = 64.0;

fn entry(id: &'static str, icon: IconName, label: &'static str, kbd: &'static str, compact: bool, cx: &mut Context<Root>, action: Box<dyn gpui::Action>) -> impl IntoElement {
  let ink = ink(cx);
  let action = std::rc::Rc::new(action);
  div()
    .id(id)
    .flex()
    .items_center()
    .gap(px(10.0))
    .px(px(10.0))
    .h(px(32.0))
    .rounded(px(8.0))
    .cursor_pointer()
    .hover(|s| s.bg(ink.hover))
    .when(compact, |d| d.justify_center().px(px(0.0)))
    .tooltip(guise::tooltip(format!("{} ({kbd})", t(label))))
    .on_click(move |_, window, cx| window.dispatch_action(action.boxed_clone(), cx))
    .child(div().text_color(ink.dimmed).child(guise::Icon::new(icon).size(Size::Sm)))
    .when(!compact, |d| {
      d.child(div().flex_1().child(t(label))).child(div().text_size(px(11.0)).text_color(ink.dimmed).child(kbd))
    })
}

pub fn render(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) -> impl IntoElement {
  let ink = ink(cx);
  let compact = root.compact;
  let width = if compact { COMPACT } else { WIDTH };
  let items = root.snap.visible();
  let mut list = div().id("sidebar-list").flex().flex_col().gap(px(2.0)).flex_1().overflow_y_scroll().px(px(8.0)).pb(px(8.0));

  let mut last_section: Option<Option<String>> = None;
  let any_sections = !root.snap.sections.is_empty();
  let mut shown_pinned = false;
  for (i, it) in items.iter().enumerate() {
    let pinned = root.snap.pinned(it);
    if pinned && !shown_pinned && !compact {
      shown_pinned = true;
      list = list.child(header(t("Pinned"), None, cx));
    }
    if !pinned && any_sections && !compact {
      let sec = root.snap.section_of(it);
      if last_section.as_ref() != Some(&sec) {
        last_section = Some(sec.clone());
        let (name, id) = match &sec {
          Some(id) => (root.snap.sections.iter().find(|s| &s.id == id).map(|s| s.name.clone()).unwrap_or_default(), Some(id.clone())),
          None => (t("Unassigned").to_string(), None),
        };
        list = list.child(header(name, id, cx));
      }
    }
    list = list.child(row::render(root, it, i, window, cx));
  }
  if items.is_empty() && !compact {
    list = list.child(div().p(px(12.0)).text_color(ink.dimmed).text_size(px(13.0)).child(t("No Bots yet. Press New to create one.")));
  }

  let hidden: Vec<store::Bot> = root.snap.hidden().into_iter().cloned().collect();
  let hidden_label: SharedString = if items.is_empty() && !hidden.is_empty() {
    t("Show Hidden Bots").into()
  } else {
    format!("{} ({})", t("Hidden Bots"), hidden.len()).into()
  };
  let mut footer = div().flex().flex_col().px(px(8.0)).pb(px(10.0)).gap(px(2.0));
  if !hidden.is_empty() && !compact {
    footer = footer.child(
      div()
        .id("hidden-toggle")
        .px(px(10.0))
        .h(px(28.0))
        .flex()
        .items_center()
        .rounded(px(8.0))
        .cursor_pointer()
        .text_size(px(12.0))
        .text_color(ink.dimmed)
        .hover(|s| s.bg(ink.hover))
        .on_click(cx.listener(|this, _, _, cx| {
          this.show_hidden = !this.show_hidden;
          cx.notify();
        }))
        .child(hidden_label),
    );
    if root.show_hidden {
      for b in hidden {
        let item = Item::Bot(b.id.clone());
        footer = footer.child(row::render(root, &item, usize::MAX, window, cx));
      }
    }
  }
  footer = footer.child(crate::account::render(root, compact, cx));

  div()
    .w(px(width))
    .h_full()
    .flex_none()
    .flex()
    .flex_col()
    .bg(ink.sidebar)
    .border_r_1()
    .border_color(ink.border)
    .child(
      div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .p(px(8.0))
        .child(entry("new", IconName::SquarePen, "New", "⌘N", compact, cx, Box::new(crate::actions::NewChat)))
        .child(entry("search", IconName::Search, "Search", "⌘K", compact, cx, Box::new(crate::actions::Palette)))
        .child(entry("market", IconName::Store, "Marketplace", "⇧⌘M", compact, cx, Box::new(crate::actions::Marketplace))),
    )
    .children(crate::sidebar::row::team_card(root, compact, cx))
    .child(list)
    .child(footer)
}

fn header(name: impl Into<SharedString>, section: Option<String>, cx: &mut Context<Root>) -> impl IntoElement {
  let ink = ink(cx);
  let name: SharedString = name.into();
  let id: SharedString = format!("section-{}", section.clone().unwrap_or_default()).into();
  div()
    .id(id)
    .px(px(10.0))
    .pt(px(10.0))
    .pb(px(4.0))
    .text_size(px(11.0))
    .font_weight(gpui::FontWeight::SEMIBOLD)
    .text_color(ink.dimmed)
    .when_some(section, |d, s| {
      d.on_mouse_down(
        gpui::MouseButton::Right,
        cx.listener(move |this, ev: &gpui::MouseDownEvent, w, cx| menu::section(this, &s, ev.position, w, cx)),
      )
    })
    .child(name.to_uppercase())
}
