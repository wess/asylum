//! Tasks → Routines: each routine's schedule or Paused state with an inline
//! switch; the detail view shows Instruction, When to run, and Webhook, with
//! Pause/Resume, Test, Edit, Delete routine, and run history.

use super::{section, Details};
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString, Window};
use guise::{ActionIcon, Badge, Button, Group, IconName, Size, Switch, Variant};

fn when_ms(d: &Details, ms: Option<i64>) -> String {
  let tz = d.rt.tz();
  ms.and_then(chrono::DateTime::from_timestamp_millis)
    .map(|t| t.with_timezone(&tz).format("%a %b %-d, %-I:%M %p").to_string())
    .unwrap_or_else(|| t("On its next event").into())
}

fn copyable(label: &'static str, value: String, ink: &crate::theme::Ink) -> impl IntoElement {
  let v = value.clone();
  div()
    .id(label)
    .flex()
    .flex_col()
    .gap(px(2.0))
    .p(px(8.0))
    .rounded(px(6.0))
    .bg(ink.surface)
    .cursor_pointer()
    .tooltip(guise::tooltip(t("Click to copy")))
    .on_click(move |_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(v.clone())))
    .child(div().text_size(px(11.0)).text_color(ink.dimmed).child(t(label)))
    .child(div().text_size(px(12.0)).font_family("Menlo").child(SharedString::from(value)))
}

pub fn render(d: &mut Details, window: &mut Window, cx: &mut Context<Details>) -> AnyElement {
  let ink = ink(cx);
  if let Some(id) = d.routine.clone() {
    if let Some(r) = d.data.routines.iter().find(|r| r.id == id).cloned() {
      return detail(d, r, window, cx);
    }
  }
  let mut col = div().flex().flex_col().gap(px(6.0)).child(section(t("Routines"), cx));
  if d.data.routines.is_empty() {
    col = col.child(
      div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(div().text_size(px(13.0)).text_color(ink.dimmed).child(t("Ask in chat to set a routine")))
        .child(Button::new("add-routine", t("Add routine")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, w, cx| prefill(this, "Set up a routine to ", w, cx)))),
    );
  }
  for r in d.data.routines.clone() {
    let (id, id2, on) = (r.id.clone(), r.id.clone(), r.active);
    let when = if r.active { schedule::describe(&r.trigger, &r.schedule, &r.filter) } else { t("Paused").into() };
    col = col.child(
      div()
        .id(SharedString::from(format!("r-{}", r.id)))
        .flex()
        .items_center()
        .gap(px(10.0))
        .p(px(10.0))
        .rounded(px(8.0))
        .bg(ink.surface)
        .cursor_pointer()
        .hover(|s| s.bg(ink.hover))
        .on_click(cx.listener(move |this, _, _, cx| {
          this.routine = Some(id.clone());
          cx.notify();
        }))
        .child(div().flex_1().min_w_0().flex().flex_col().child(div().truncate().child(r.name.clone())).child(div().truncate().text_size(px(12.0)).text_color(ink.dimmed).child(when)))
        .child(Switch::new(SharedString::from(format!("rs-{}", r.id))).checked(on).color(guise::ColorName::Violet).on_change(cx.listener(move |this, _, _, cx| {
          let rt = this.rt.clone();
          let id = id2.clone();
          this.run(cx, async move { agent::api::bots::set_routine_active(&rt, &id, !on).await }, |this, _, cx| this.load(cx));
        }))),
    );
  }
  if !d.data.routines.is_empty() {
    col = col.child(Button::new("add-routine2", t("Add routine")).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(|this, _, w, cx| prefill(this, "Set up a routine to ", w, cx))));
  }
  col.child(div().pt(px(8.0)).text_size(px(11.5)).text_color(ink.dimmed).child(crate::i18n::tf("Up to {} routines per Agent.", &[&store::routines::LIMIT_PER_BOT.to_string()]))).into_any_element()
}

/// Put text into this chat's composer.
fn prefill(d: &mut Details, text: &str, window: &mut Window, cx: &mut Context<Details>) {
  let text = t(Box::leak(text.to_string().into_boxed_str())).to_string();
  let _ = d.root.update(cx, |r, cx| {
    if let Some(p) = &r.pane {
      p.update(cx, |p, cx| p.composer.update(cx, |c, cx| c.prefill(&text, window, cx)));
    }
  });
}

fn detail(d: &mut Details, r: store::Routine, window: &mut Window, cx: &mut Context<Details>) -> AnyElement {
  let ink = ink(cx);
  let _ = window;
  let when = schedule::describe(&r.trigger, &r.schedule, &r.filter);
  let url = agent::webhook::url(d.rt.settings().webhook_port, &r.id);
  let runs: Vec<store::Run> = d.data.runs.iter().filter(|x| x.routine_id.as_deref() == Some(r.id.as_str())).take(store::runs::HISTORY as usize).cloned().collect();
  let testing = d.testing.as_deref() == Some(r.id.as_str()) && d.rt.queues.busy(&r.bot_id);
  let (id1, id2, id3, name, on) = (r.id.clone(), r.id.clone(), r.id.clone(), r.name.clone(), r.active);
  let mut col = div()
    .flex()
    .flex_col()
    .gap(px(8.0))
    .pt(px(10.0))
    .child(
      div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(ActionIcon::new("back-routines", IconName::ChevronLeft).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(|this, _, _, cx| {
          this.routine = None;
          cx.notify();
        })))
        .child(div().flex_1().font_weight(gpui::FontWeight::SEMIBOLD).child(r.name.clone()))
        .child(Badge::new(if on { t("Active") } else { t("Paused") }).size(Size::Xs).color(if on { guise::ColorName::Green } else { guise::ColorName::Gray })),
    )
    .child(section(t("Instruction"), cx))
    .child(div().text_size(px(13.0)).child(SharedString::from(r.instruction.clone())))
    .child(section(t("When to run"), cx))
    .child(div().text_size(px(13.0)).child(SharedString::from(when)))
    .when(r.active && r.next_run.is_some(), |c| c.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(crate::i18n::tf("Next run: {}", &[&when_ms(d, r.next_run)]))));
  col = col
    .child(section(t("Webhook"), cx))
    .child(copyable("POST to", url, &ink))
    .child(copyable("Key", r.webhook_key.clone(), &ink))
    .child(copyable("Header", format!("Authorization: Bearer {}", r.webhook_key), &ink))
    .child(div().text_size(px(11.5)).text_color(ink.dimmed).child(t("The JSON body is passed along with the instruction. A 200 response means a run started.")));
  col = col.child(div().pt(px(8.0)).child(
    Group::new()
      .gap(Size::Xs)
      .wrap(true)
      .child(Button::new("pause", if on { t("Pause") } else { t("Resume") }).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| {
        let rt = this.rt.clone();
        let id = id1.clone();
        this.run(cx, async move { agent::api::bots::set_routine_active(&rt, &id, !on).await }, |this, _, cx| this.load(cx));
      })))
      .child(Button::new("test", if testing { t("Running…") } else { t("Test") }).size(Size::Xs).variant(Variant::Light).disabled(testing).on_click(cx.listener(move |this, _, _, cx| {
        let rt = this.rt.clone();
        let id = id2.clone();
        this.testing = Some(id.clone());
        this.run(cx, async move { agent::api::bots::test_routine(&rt, &id).await.map_err(|_| anyhow::anyhow!(t("Couldn't start a test run"))) }, |_, _, cx| cx.notify());
      })))
      .child(Button::new("edit", t("Edit")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(move |this, _, w, cx| {
        let text = format!("{} {}: ", t("Edit your routine:"), name);
        let _ = this.root.update(cx, |r, cx| {
          if let Some(p) = &r.pane {
            p.update(cx, |p, cx| p.composer.update(cx, |c, cx| c.prefill(&text, w, cx)));
          }
        });
      })))
      .child(Button::new("delete", t("Delete routine")).size(Size::Xs).variant(Variant::Subtle).color(guise::ColorName::Red).left_section(guise::Icon::new(IconName::Trash2).size(Size::Xs)).on_click(cx.listener(move |this, _, w, cx| {
        let id = id3.clone();
        let me = cx.entity().downgrade();
        let _ = this.root.update(cx, |r, cx| {
          crate::root::dialogs::confirm(r, t("Delete this routine?"), t("There's no undo."), t("Delete routine"), w, cx, move |r, _, cx| {
            let rt = r.rt.clone();
            let id = id.clone();
            let me = me.clone();
            r.run(cx, async move { agent::api::bots::delete_routine(&rt, &id).await }, move |_, _, cx| {
              let _ = me.update(cx, |d, cx| {
                d.routine = None;
                d.load(cx);
              });
            });
          });
        });
      }))),
  ));
  col = col.child(section(t("Run history"), cx));
  if runs.is_empty() {
    col = col.child(div().text_size(px(13.0)).text_color(ink.dimmed).child(t("No runs yet")));
  }
  for run in runs {
    let (label, color) = match run.status.as_str() {
      "running" | "waiting" => (t("Running"), guise::ColorName::Blue),
      "done" => (t("Succeeded"), guise::ColorName::Green),
      _ => (t("Failed"), guise::ColorName::Red),
    };
    col = col.child(
      div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .py(px(6.0))
        .border_b_1()
        .border_color(ink.border)
        .child(div().flex().items_center().gap(px(8.0)).child(Badge::new(label).size(Size::Xs).color(color)).child(div().text_size(px(12.0)).text_color(ink.dimmed).child(when_ms(d, Some(run.started)))).when(run.origin == "test", |x| x.child(Badge::new(t("Test")).size(Size::Xs).variant(Variant::Light))))
        .when(!run.error.is_empty(), |x| x.child(div().text_size(px(12.0)).text_color(ink.danger).child(SharedString::from(run.error.clone()))))
        .when(!run.summary.is_empty(), |x| x.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(SharedString::from(run.summary.chars().take(160).collect::<String>())))),
    );
  }
  col.into_any_element()
}
