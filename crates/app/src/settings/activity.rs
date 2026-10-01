//! Activity: Conversation Insights for the last week, Action Recording and
//! the audit log, conversation export, OpenTelemetry, and the Admin API.

use super::{heading, row, Dialog};
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, PathPromptOptions, SharedString};
use guise::{Button, Size, Variant};
use store::events::Event;

pub const WEEK_MS: i64 = 7 * 24 * 3600 * 1000;
pub const ADMIN_PORT: u16 = 7438;

pub struct Data {
  pub insights: agent::api::insights::Insights,
  pub actions: Vec<Event>,
  pub audit: Vec<Event>,
}

pub async fn load(rt: &agent::Runtime) -> anyhow::Result<Data> {
  let since = store::now() - WEEK_MS;
  Ok(Data {
    insights: agent::api::insights::since(rt, since).await?,
    actions: store::events::list(&rt.pool, store::events::ACTION, 0, 40).await?,
    audit: store::events::list(&rt.pool, store::events::AUDIT, 0, 40).await?,
  })
}

fn when(ms: i64) -> String {
  chrono::DateTime::from_timestamp_millis(ms).map(|d| d.with_timezone(&chrono::Local).format("%b %-d, %H:%M").to_string()).unwrap_or_default()
}

fn log(rows: &[Event], cx: &Context<Dialog>) -> AnyElement {
  let ink = ink(cx);
  if rows.is_empty() {
    return div().text_size(px(12.5)).text_color(ink.dimmed).py(px(6.0)).child(t("Nothing yet.")).into_any_element();
  }
  let mut col = div().flex().flex_col();
  for e in rows {
    let label = if e.target.is_empty() { e.name.clone() } else { format!("{} · {}", e.name, e.target) };
    col = col.child(
      div()
        .flex()
        .gap(px(10.0))
        .py(px(3.0))
        .text_size(px(12.0))
        .child(div().w(px(110.0)).flex_none().text_color(ink.dimmed).child(when(e.at)))
        .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis().child(SharedString::from(label)))
        .child(div().flex_none().text_color(ink.dimmed).child(SharedString::from(e.outcome.clone()))),
    );
  }
  col.into_any_element()
}

pub fn render(d: &mut Dialog, cx: &mut Context<Dialog>) -> AnyElement {
  let ink = ink(cx);
  let s = d.rt.settings();
  let mut col = div().flex().flex_col().child(heading(t("Insights (last 7 days)"), cx));
  if let Some(a) = &d.activity {
    let i = &a.insights;
    let stat = |n: i64, label: &'static str| div().flex().flex_col().p(px(10.0)).rounded(px(8.0)).bg(ink.surface).min_w(px(120.0)).child(div().text_size(px(20.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(n.to_string())).child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t(label)));
    col = col.child(
      div()
        .flex()
        .flex_wrap()
        .gap(px(8.0))
        .py(px(6.0))
        .child(stat(i.conversations, "Conversations"))
        .child(stat(i.messages, "Messages"))
        .child(stat(i.actions, "Actions"))
        .child(stat(i.outcomes.get("denied").copied().unwrap_or(0) + i.outcomes.get("user-denied").copied().unwrap_or(0), "Stopped by approvals")),
    );
    let top = |list: &[(String, i64)]| list.iter().take(5).map(|(n, c)| format!("{n} ({c})")).collect::<Vec<_>>().join(", ");
    if !i.tools.is_empty() {
      col = col.child(row(t("Most used tools"), Some(&top(&i.tools)), div(), cx));
    }
    if !i.bots.is_empty() {
      col = col.child(row(t("Busiest Bots"), Some(&top(&i.bots)), div(), cx));
    }
    col = col.child(heading(t("Action Recording"), cx)).child(div().text_size(px(12.0)).text_color(ink.dimmed).pb(px(4.0)).child(t("What Bots did: tool, target, and outcome only. Kept for 90 days.")));
    col = col.child(log(&a.actions, cx));
    col = col.child(heading(t("Audit log"), cx)).child(log(&a.audit, cx));
  }
  col = col
    .child(heading(t("Export"), cx))
    .child(row(
      t("Export all conversations"),
      Some(t("Prompts, responses, and tool input and output, as JSON and Markdown.")),
      Button::new("export-all", t("Export…")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, _, cx| {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some(t("Export here").into()) });
        let rt = this.rt.clone();
        cx.spawn(async move |this, cx| {
          let Ok(Ok(Some(dirs))) = rx.await else { return };
          let Some(dir) = dirs.into_iter().next() else { return };
          let r = crate::tk::run(async move { agent::api::export::all(&rt, &dir.join("asylum.export")).await }).await;
          let _ = this.update(cx, |this, cx| {
            this.status = Some(match r {
              Ok(files) => crate::i18n::tf("Exported {} conversations.", &[&files.len().to_string()]),
              Err(e) => e.to_string(),
            });
            cx.notify();
          });
        })
        .detach();
      })),
      cx,
    ))
    .child(heading(t("OpenTelemetry"), cx))
    .child(row(
      t("Collector endpoint"),
      Some(if d.rt.policy().otel_endpoint.is_empty() { t("Send turn and tool spans (ids and names only, no content) over OTLP/HTTP. Empty: off.") } else { t("Set by your admin.") }),
      div().flex().gap(px(6.0)).child(div().w(px(220.0)).child(d.otel.clone())).child(Button::new("otel-save", t("Save")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, _, cx| {
        let v = this.otel.read(cx).text().trim().to_string();
        super::save(&this.rt, |s| s.otel_endpoint = v);
        this.status = Some(t("Saved.").into());
        cx.notify();
      }))),
      cx,
    ))
    .child(heading(t("Admin API"), cx))
    .child(row(
      t("Local Admin API"),
      Some(&if s.admin_port == 0 { t("Read Bots, actions, the audit log, insights, and conversations over HTTP on this Mac, with a token.").to_string() } else { format!("http://127.0.0.1:{}/v1 · {}", s.admin_port, t("Turning it off takes effect after a restart.")) }),
      guise::Switch::new("admin-api").checked(s.admin_port != 0).color(guise::ColorName::Violet).on_change({
        let rt = d.rt.clone();
        let on = s.admin_port == 0;
        move |_, _, cx| {
          super::save(&rt, |s| s.admin_port = if on { ADMIN_PORT } else { 0 });
          if on {
            let rt = rt.clone();
            crate::tk::spawn(async move { agent::admin::serve(rt).await });
          }
          cx.refresh_windows();
        }
      }),
      cx,
    ));
  if s.admin_port != 0 {
    col = col.child(row(
      t("Token"),
      Some(t("Send it as Authorization: Bearer <token>.")),
      Button::new("admin-token", t("Copy token")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, _, cx| {
        match agent::admin::token() {
          Ok(tok) => {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(tok));
            this.status = Some(t("Token copied.").into());
          }
          Err(e) => this.status = Some(e.to_string()),
        }
        cx.notify();
      })),
      cx,
    ));
  }
  if let Some(st) = &d.status {
    col = col.child(div().pt(px(8.0)).text_size(px(12.0)).text_color(ink.primary).child(SharedString::from(st.clone())));
  }
  col.into_any_element()
}
