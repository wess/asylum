//! Cards: how each message part renders, and the inputs cards own.

pub mod approval;
pub mod data;
pub mod draft;
pub mod form;
pub mod media;
pub mod secret;
pub mod skill;
pub mod small;
pub mod takeover;
pub mod tool;

use crate::chat::ChatPane;
use crate::theme::ink;
use agent::Part;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, App, Context, Entity, Window};
use guise::{TextArea, TextInput};
use std::collections::HashMap;
use store::Message;

/// Text inputs owned by cards, keyed by "<message>:<part>:<field>".
#[derive(Default)]
pub struct Inputs {
  pub lines: HashMap<String, Entity<TextInput>>,
  pub areas: HashMap<String, Entity<TextArea>>,
}

impl Inputs {
  pub fn line(&mut self, key: String, init: &str, password: bool, cx: &mut Context<ChatPane>) -> Entity<TextInput> {
    self
      .lines
      .entry(key)
      .or_insert_with(|| cx.new(|cx| TextInput::new(cx).value(init).password(password).size(guise::Size::Sm)))
      .clone()
  }

  pub fn area(&mut self, key: String, init: &str, rows: usize, cx: &mut Context<ChatPane>) -> Entity<TextArea> {
    self
      .areas
      .entry(key)
      .or_insert_with(|| cx.new(|cx| TextArea::new(cx).value(init).rows(rows).max_rows(16)))
      .clone()
  }

  pub fn text(&self, key: &str, cx: &App) -> String {
    self
      .lines
      .get(key)
      .map(|e| e.read(cx).text())
      .or_else(|| self.areas.get(key).map(|e| e.read(cx).text()))
      .unwrap_or_default()
  }
}

pub fn key(m: &Message, i: usize, field: &str) -> String {
  format!("{}:{i}:{field}", m.id)
}

/// A bordered card shell.
pub fn shell(cx: &App) -> gpui::Div {
  let ink = ink(cx);
  div()
    .flex()
    .flex_col()
    .gap(px(8.0))
    .p(px(12.0))
    .rounded(px(10.0))
    .border_1()
    .border_color(ink.border)
    .bg(ink.surface)
    .max_w(px(640.0))
}

pub fn render(pane: &mut ChatPane, m: &Message, i: usize, p: &Part, window: &mut Window, cx: &mut Context<ChatPane>) -> AnyElement {
  match p {
    Part::Tool { .. } => tool::render(pane, m, i, p, cx),
    Part::Reasoning { text } => small::reasoning(pane, m, i, text, cx),
    Part::Approval { id } => approval::render(pane, id, cx),
    Part::Question { text, options } => small::question(pane, m, text, options, cx),
    Part::SecretRequest { .. } => secret::render(pane, m, i, p, cx),
    Part::Takeover { reason, status } => takeover::render(pane, m, reason, status, cx),
    Part::Form { .. } => form::render(pane, m, i, p, window, cx),
    Part::Draft { .. } => draft::render(pane, m, i, p, cx),
    Part::Attachment { name, path, mime, size } => media::file(pane, name, path, mime, Some(*size), cx),
    Part::File { name, path, mime } => media::file(pane, name, path, mime, None, cx),
    Part::Image { path, caption } => media::image(pane, path, caption, cx),
    Part::Link { url, title } => media::link(pane, url, title, cx),
    Part::Card { kind, title, data } => data::render(kind, title, data, cx),
    Part::Handoff { from, to, direction } => small::handoff(from, to, direction, cx),
    Part::Routine { id, name, event } => small::routine(pane, id, name, event, cx),
    Part::VoiceChat { seconds, transcript } => media::voicechat(pane, m, *seconds, transcript, cx),
    Part::VoiceMemo { path, transcript } => media::memo(pane, m, path, transcript, cx),
    Part::SkillDraft { .. } => skill::render(pane, m, i, p, cx),
    Part::Connect { plugin, status } => small::connect(plugin, status, cx),
    Part::Error { text } => small::error(pane, text, cx),
  }
}
