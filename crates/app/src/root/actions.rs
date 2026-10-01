//! Keyboard and menu commands handled by the main window.

use super::{Right, Root};
use crate::actions::*;
use crate::state::Item;
use gpui::prelude::*;
use gpui::{px, Context, Stateful, Div, Window};

pub fn wire(root: Stateful<Div>, cx: &mut Context<Root>) -> Stateful<Div> {
  root
    .on_action(cx.listener(|this, _: &Palette, w, cx| crate::palette::open(this, None, w, cx)))
    .on_action(cx.listener(|this, _: &SearchBots, w, cx| crate::palette::open(this, Some(agent::api::search::Scope::Bots), w, cx)))
    .on_action(cx.listener(|this, _: &NewChat, w, cx| crate::newchat::open(this, w, cx)))
    .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
      this.compact = !this.compact;
      let c = this.compact;
      crate::settings::save(&this.rt, |s| s.compact_sidebar = c);
      cx.notify();
    }))
    .on_action(cx.listener(|this, _: &FocusPrompt, w, cx| {
      if let Some(p) = &this.pane {
        p.update(cx, |p, cx| p.focus_composer(w, cx));
      }
    }))
    .on_action(cx.listener(|this, _: &FindInChat, w, cx| {
      if let Some(p) = &this.pane {
        p.update(cx, |p, cx| p.toggle_find(w, cx));
      }
    }))
    .on_action(cx.listener(|this, _: &Bot1, w, cx| this.jump(0, w, cx)))
    .on_action(cx.listener(|this, _: &Bot2, w, cx| this.jump(1, w, cx)))
    .on_action(cx.listener(|this, _: &Bot3, w, cx| this.jump(2, w, cx)))
    .on_action(cx.listener(|this, _: &Bot4, w, cx| this.jump(3, w, cx)))
    .on_action(cx.listener(|this, _: &Bot5, w, cx| this.jump(4, w, cx)))
    .on_action(cx.listener(|this, _: &Bot6, w, cx| this.jump(5, w, cx)))
    .on_action(cx.listener(|this, _: &Bot7, w, cx| this.jump(6, w, cx)))
    .on_action(cx.listener(|this, _: &Bot8, w, cx| this.jump(7, w, cx)))
    .on_action(cx.listener(|this, _: &Bot9, w, cx| this.jump(8, w, cx)))
    .on_action(cx.listener(|this, _: &PrevBot, w, cx| this.step(-1, w, cx)))
    .on_action(cx.listener(|this, _: &NextBot, w, cx| this.step(1, w, cx)))
    .on_action(cx.listener(|this, _: &CyclePrev, w, cx| this.step(-1, w, cx)))
    .on_action(cx.listener(|this, _: &CycleNext, w, cx| this.step(1, w, cx)))
    .on_action(cx.listener(|this, _: &Back, w, cx| {
      if let Some(c) = this.history.back(this.active.as_deref()) {
        this.goto(&c, w, cx);
      }
    }))
    .on_action(cx.listener(|this, _: &Forward, w, cx| {
      if let Some(c) = this.history.forward(this.active.as_deref()) {
        this.goto(&c, w, cx);
      }
    }))
    .on_action(cx.listener(|this, _: &Marketplace, w, cx| crate::market::open(this, w, cx)))
    .on_action(cx.listener(|this, _: &OpenSettings, w, cx| crate::settings::open(this, None, w, cx)))
    .on_action(cx.listener(|this, _: &ToggleBotSettings, w, cx| {
      if this.right == Right::Details {
        this.close_right(cx);
      } else {
        this.show_details(w, cx);
      }
    }))
    .on_action(cx.listener(|this, _: &Details, w, cx| {
      if this.right == Right::Details {
        this.close_right(cx);
      } else {
        this.show_details(w, cx);
      }
    }))
    .on_action(cx.listener(|this, _: &OpenComputer, w, cx| {
      if this.right == Right::Computer {
        this.close_right(cx);
      } else {
        this.show_computer(w, cx);
      }
    }))
    .on_action(cx.listener(|this, _: &OpenBrowser, w, cx| {
      if this.right == Right::Web {
        this.close_right(cx);
      } else {
        this.show_web(None, w, cx);
      }
    }))
    .on_action(cx.listener(|this, _: &OpenTerminal, w, cx| {
      if this.right != Right::Computer {
        this.show_computer(w, cx);
      }
      if let Some(p) = this.panel.clone() {
        p.update(cx, |p, cx| p.show_terminal(w, cx));
      }
    }))
    .on_action(cx.listener(|this, _: &SendAnywhere, w, cx| {
      if let Some(p) = &this.pane {
        p.update(cx, |p, cx| p.send(w, cx));
      }
    }))
    .on_action(cx.listener(|this, _: &Dictate, w, cx| {
      if let Some(p) = &this.pane {
        p.update(cx, |p, cx| p.dictate(w, cx));
      }
    }))
    .on_action(cx.listener(|this, _: &StopBot, _, cx| {
      if let Some(c) = this.active.clone() {
        let rt = this.rt.clone();
        this.run(cx, async move { agent::api::chat::stop(&rt, &c).await }, |_, _, _| {});
      }
    }))
    .on_action(cx.listener(|this, _: &ZoomIn, w, cx| this.zoom_by(0.1, w, cx)))
    .on_action(cx.listener(|this, _: &ZoomOut, w, cx| this.zoom_by(-0.1, w, cx)))
    .on_action(cx.listener(|this, _: &ZoomReset, w, cx| {
      this.zoom = 1.0;
      this.zoom_by(0.0, w, cx)
    }))
    .on_action(cx.listener(|_, _: &Fullscreen, w, _| w.toggle_fullscreen()))
    .on_action(cx.listener(|_, _: &CloseWindow, w, _| w.remove_window()))
    .on_action(cx.listener(|this, _: &Cancel, w, cx| this.cancel(w, cx)))
    .on_action(cx.listener(|this, _: &About, w, cx| crate::settings::about(this, w, cx)))
}

impl Root {
  /// Navigate by history without recording a new visit.
  fn goto(&mut self, chat: &str, window: &mut Window, cx: &mut Context<Self>) {
    let saved = std::mem::take(&mut self.history);
    self.active = None;
    self.open(chat, window, cx);
    self.history = saved;
  }

  pub fn jump(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
    let items: Vec<Item> = self.snap.visible();
    if let Some(it) = items.get(i) {
      self.open_item(&it.clone(), window, cx);
    }
  }

  pub fn step(&mut self, by: i32, window: &mut Window, cx: &mut Context<Self>) {
    let items = self.snap.visible();
    if items.is_empty() {
      return;
    }
    let cur = self.active_item().and_then(|a| items.iter().position(|i| *i == a));
    let n = items.len() as i32;
    let next = match cur {
      Some(c) => ((c as i32 + by).rem_euclid(n)) as usize,
      None => 0,
    };
    self.open_item(&items[next].clone(), window, cx);
  }

  fn zoom_by(&mut self, d: f32, window: &mut Window, cx: &mut Context<Self>) {
    self.zoom = (self.zoom + d).clamp(0.6, 2.0);
    window.set_rem_size(px(16.0 * self.zoom));
    let z = self.zoom;
    crate::settings::save(&self.rt, |s| s.zoom = z);
    cx.notify();
  }

  fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.rename.is_some() {
      self.rename = None;
    } else if self.spotlight.is_some() {
      self.spotlight = None;
    } else if self.modal.is_some() {
      self.close_modal(window, cx);
      return;
    } else if let Some(p) = &self.pane {
      p.update(cx, |p, cx| p.escape(window, cx));
    }
    cx.notify();
  }
}
