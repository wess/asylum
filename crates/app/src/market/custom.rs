//! Adding a custom MCP server: a Remote HTTPS URL (optionally signed in per
//! account) or a Command run on this computer.

use super::Market;
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, Entity, Window};
use guise::{Button, Checkbox, SegmentedControl, Size, TextInput};

#[derive(Clone)]
pub struct Form {
  pub kind: Entity<SegmentedControl>,
  pub name: Entity<TextInput>,
  pub url: Entity<TextInput>,
  pub command: Entity<TextInput>,
  pub oauth: bool,
}

impl Form {
  pub fn new(window: &mut Window, cx: &mut Context<Market>) -> Self {
    let name = cx.new(|cx| TextInput::new(cx).placeholder(t("Name")).size(Size::Sm));
    window.focus(&name.read(cx).focus_handle(), cx);
    Self {
      kind: cx.new(|cx| SegmentedControl::new(cx).data([t("Remote HTTPS"), t("Command")]).selected(0).size(Size::Sm)),
      name,
      url: cx.new(|cx| TextInput::new(cx).placeholder("https://mcp.example.com/mcp").size(Size::Sm)),
      command: cx.new(|cx| TextInput::new(cx).placeholder("npx -y @scope/server --flag").size(Size::Sm)),
      oauth: false,
    }
  }
}

pub fn render(m: &mut Market, f: Form, cx: &mut Context<Market>) -> AnyElement {
  let ink = ink(cx);
  let remote = f.kind.read(cx).selected_index() == 0;
  let mut col = div().flex().flex_col().gap(px(8.0)).max_w(px(520.0)).child(div().text_size(px(16.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(t("Custom MCP server"))).child(f.kind.clone()).child(f.name.clone());
  if remote {
    let on = f.oauth;
    col = col.child(f.url.clone()).child(
      div()
        .id("oauth")
        .flex()
        .items_center()
        .gap(px(6.0))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
          if let Some(c) = &mut this.custom {
            c.oauth = !on;
          }
          cx.notify();
        }))
        .child(Checkbox::new("oauth-ck").checked(on))
        .child(div().text_size(px(13.0)).child(t("Sign in with OAuth (each account signs in once)"))),
    );
  } else {
    col = col.child(f.command.clone()).child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("Runs directly (not through a shell) in the computer's workspace.")));
  }
  let _ = m;
  col
    .child(Button::new("add-custom", t("Add")).size(Size::Sm).on_click(cx.listener(move |this, _, _, cx| {
      let name = f.name.read(cx).text().trim().to_string();
      if name.is_empty() {
        this.status = Some(t("Give it a name.").into());
        cx.notify();
        return;
      }
      let rt = this.rt.clone();
      let (url, command) = if remote { (Some(f.url.read(cx).text()), None) } else { (None, Some(f.command.read(cx).text())) };
      let oauth = f.oauth;
      this.run(cx, async move {
        let (cmd, args) = match command {
          Some(c) => {
            let mut parts = c.split_whitespace().map(str::to_string);
            (parts.next(), parts.collect::<Vec<_>>())
          }
          None => (None, Vec::new()),
        };
        agent::api::connect::add_custom(&rt, &name, url.as_deref(), cmd.as_deref(), &args, oauth).await
      }, |this, p, cx| {
        this.custom = None;
        this.open = Some(p.id);
        this.load(cx);
      });
    })))
    .into_any_element()
}
