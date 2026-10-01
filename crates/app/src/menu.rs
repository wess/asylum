//! Context menus. Entries are collected first, then the guise ContextMenu
//! entity is built from them and shown at the pointer.

use gpui::{App, AppContext, Entity, Pixels, Point, SharedString, Window};
use guise::ContextMenu;

pub type Handler = Box<dyn Fn(&mut Window, &mut App) + 'static>;

pub enum Entry {
  Item(SharedString, Handler),
  Danger(SharedString, Handler),
  Section(SharedString),
  Divider,
}

#[derive(Default)]
pub struct Menu {
  pub entries: Vec<Entry>,
}

impl Menu {
  pub fn item(mut self, label: impl Into<SharedString>, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    self.entries.push(Entry::Item(label.into(), Box::new(f)));
    self
  }

  pub fn danger(mut self, label: impl Into<SharedString>, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    self.entries.push(Entry::Danger(label.into(), Box::new(f)));
    self
  }

  pub fn section(mut self, label: impl Into<SharedString>) -> Self {
    self.entries.push(Entry::Section(label.into()));
    self
  }

  pub fn divider(mut self) -> Self {
    self.entries.push(Entry::Divider);
    self
  }

  pub fn show(self, at: Point<Pixels>, width: f32, window: &mut Window, cx: &mut App) -> Entity<ContextMenu> {
    let entity = cx.new(|cx| {
      let mut m = ContextMenu::new(cx).width(width);
      for e in self.entries {
        m = match e {
          Entry::Item(l, f) => m.item(l, f),
          Entry::Danger(l, f) => m.danger_item(l, f),
          Entry::Section(l) => m.section(l),
          Entry::Divider => m.divider(),
        };
      }
      m
    });
    entity.update(cx, |m, cx| m.show(at, window, cx));
    entity
  }
}
