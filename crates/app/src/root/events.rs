use super::Root;
use agent::Event;
use gpui::{Context, Window};

impl Root {
  pub fn on_event(&mut self, e: Event, window: &mut Window, cx: &mut Context<Self>) {
    match &e {
      Event::Plugins | Event::Skills => {
        if let Some(m) = self.market.clone() {
          m.update(cx, |m, cx| m.on_plugins(cx));
        }
        self.reload(cx);
      }
      Event::BotsChanged | Event::ChatsChanged | Event::Usage => self.reload(cx),
      Event::Approval { .. } | Event::Routines { .. } => self.reload(cx),
      Event::BotStatus { bot, status } => {
        if let Some(b) = self.snap.bots.iter_mut().find(|b| &b.id == bot) {
          b.status = status.clone();
        }
        cx.notify();
      }
      Event::Notify { title, body, .. } => {
        // Suppressed while the app is focused; the sidebar still updates.
        if !window.is_window_active() {
          crate::notify::post(title, body);
        }
        self.reload(cx);
      }
      Event::Computer { state } => {
        if let Some(d) = state.strip_prefix("disk:") {
          self.disk = d.to_string();
        } else {
          self.computer = state.clone();
        }
        cx.notify();
      }
      Event::Message { .. } | Event::Stream { .. } | Event::Notice { .. } | Event::Screen { .. } => {}
    }
    if let Some(p) = self.pane.clone() {
      p.update(cx, |p, cx| p.on_event(&e, window, cx));
    }
    if let Some(d) = self.details.clone() {
      d.update(cx, |d, cx| d.on_event(&e, cx));
    }
    if let Some(p) = self.panel.clone() {
      p.update(cx, |p, cx| p.on_event(&e, cx));
    }
    if matches!(e, Event::Message { .. }) {
      // Unread flags live in the snapshot.
      self.reload(cx);
    }
  }
}
