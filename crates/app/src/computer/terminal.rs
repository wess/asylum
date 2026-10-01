//! A terminal on the computer: a login shell in the shared workspace,
//! rendered by libsinclair's TermView.

use super::Panel;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, Window};
use libsinclair::termview::{TermOptions, TermView};
use libsinclair::SessionOptions;

pub fn ensure(p: &mut Panel, window: &mut Window, cx: &mut Context<Panel>) {
  if p.term.is_some() {
    return;
  }
  let mut opts = SessionOptions::default();
  opts.spawn.cwd = Some(p.rt.computer.workspace());
  opts.spawn.env.push(("WORKSPACE".into(), p.rt.computer.workspace().display().to_string()));
  match libsinclair::Session::spawn(opts) {
    Ok((session, events)) => {
      let session = std::sync::Arc::new(session);
      p.term = Some(cx.new(|cx| TermView::new(session, events, TermOptions::default(), window, cx)));
    }
    Err(e) => p.status = Some(e.to_string()),
  }
}

pub fn render(p: &mut Panel, cx: &mut Context<Panel>) -> AnyElement {
  let ink = ink(cx);
  match &p.term {
    Some(t) => div().flex_1().min_h_0().rounded(px(8.0)).overflow_hidden().border_1().border_color(ink.border).child(t.clone()).into_any_element(),
    None => div().p(px(12.0)).text_color(ink.dimmed).child(t("Starting the terminal…")).into_any_element(),
  }
}
