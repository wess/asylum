//! The @ mention picker (Bots, groups, routines, apps, @everyone) and the /
//! skill picker, driven by the token being typed at the end of the text.

use super::Composer;
use crate::theme::ink;
use agent::Runtime;
use gpui::prelude::*;
use gpui::{div, px, Context, SharedString, Window};

#[derive(Clone, Debug)]
pub struct Choice {
  pub label: String,
  pub hint: String,
  pub insert: String,
}

#[derive(Clone, Debug)]
pub struct Picker {
  pub sigil: char,
  pub token: String,
  pub items: Vec<Choice>,
}

/// The trailing `@word` or `/word` being typed, if any.
pub fn token(text: &str) -> Option<(char, String)> {
  let last = text.rsplit(|c: char| c.is_whitespace()).next()?;
  let mut chars = last.chars();
  let sigil = chars.next()?;
  if sigil != '@' && sigil != '/' {
    return None;
  }
  if sigil == '/' && text.trim_start() != last && !text.ends_with(last) {
    return None;
  }
  Some((sigil, last.to_string()))
}

pub fn detect(text: &str, rt: &Runtime, chat: &str, cx: &mut Context<Composer>) -> Option<Picker> {
  let (sigil, tok) = token(text)?;
  let q = tok[1..].to_lowercase();
  let rt = rt.clone();
  let chat = chat.to_string();
  let tok2 = tok.clone();
  cx.spawn(async move |this, cx| {
    let items = crate::tk::run(async move { choices(&rt, &chat, sigil, &q).await }).await.unwrap_or_default();
    let _ = this.update(cx, |this, cx| {
      if items.is_empty() {
        this.picker = None;
      } else if let Some(p) = &mut this.picker {
        if p.token == tok2 {
          p.items = items;
        }
      }
      cx.notify();
    });
  })
  .detach();
  Some(Picker { sigil, token: tok, items: Vec::new() })
}

async fn choices(rt: &Runtime, chat: &str, sigil: char, q: &str) -> anyhow::Result<Vec<Choice>> {
  let pool = &rt.pool;
  let hit = |s: &str| q.is_empty() || s.to_lowercase().contains(q);
  let mut out = Vec::new();
  if sigil == '/' {
    for s in store::skills::all(pool).await? {
      if hit(&s.name) {
        out.push(Choice { label: s.name.clone(), hint: s.description.clone(), insert: s.name.clone() });
      }
    }
    return Ok(out);
  }
  let c = store::chats::get(pool, chat).await?;
  if c.is_group() && hit("everyone") {
    out.push(Choice { label: "everyone".into(), hint: crate::i18n::t("Everyone in this group").into(), insert: "everyone".into() });
  }
  for b in store::bots::list(pool).await? {
    if hit(&b.name) {
      out.push(Choice { label: b.name.clone(), hint: b.label.clone(), insert: b.name.clone() });
    }
  }
  for g in store::chats::groups(pool).await? {
    if hit(&g.title) {
      out.push(Choice { label: g.title.clone(), hint: crate::i18n::t("Group chat").into(), insert: g.title.clone() });
    }
  }
  for r in store::routines::all(pool).await? {
    if hit(&r.name) {
      out.push(Choice { label: r.name.clone(), hint: crate::i18n::t("Routine").into(), insert: r.name.clone() });
    }
  }
  for p in store::plugins::all(pool).await? {
    if hit(&p.name) {
      out.push(Choice { label: p.name.clone(), hint: crate::i18n::t("App").into(), insert: p.name.clone() });
    }
  }
  out.truncate(12);
  Ok(out)
}

pub fn render(c: &mut Composer, p: &Picker, _window: &mut Window, cx: &mut Context<Composer>) -> impl IntoElement {
  let ink = ink(cx);
  let mut list = div()
    .absolute()
    .bottom(px(64.0))
    .left(px(10.0))
    .w(px(360.0))
    .max_h(px(280.0))
    .flex()
    .flex_col()
    .p(px(4.0))
    .rounded(px(10.0))
    .border_1()
    .border_color(ink.border)
    .bg(ink.body)
    .shadow_lg();
  let _ = c;
  for (i, ch) in p.items.iter().enumerate() {
    let pk = p.clone();
    let ch2 = ch.clone();
    list = list.child(
      div()
        .id(SharedString::from(format!("pick-{i}")))
        .flex()
        .gap(px(8.0))
        .px(px(8.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(|s| s.bg(ink.hover))
        .when(i == 0, |d| d.bg(ink.hover))
        .on_click(cx.listener(move |this, _, w, cx| this.insert(&pk, &ch2, w, cx)))
        .child(div().text_color(ink.primary).child(format!("{}{}", p.sigil, ch.label)))
        .child(div().flex_1().truncate().text_size(px(12.0)).text_color(ink.dimmed).child(ch.hint.clone())),
    );
  }
  list
}

#[cfg(test)]
#[path = "../../tests/picker.rs"]
mod tests;
