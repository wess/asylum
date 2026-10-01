//! A plugin's page: status, Authorize (OAuth in the browser) or connect
//! with a token/environment, labeled accounts (Add Another Account), the
//! tools it offers with on/off switches, and Remove.

use super::{status_badge, Market};
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, Entity, SharedString, Window};
use guise::{Button, Group, Size, Switch, TextInput, Variant};
use std::collections::HashMap;

pub struct State {
  pub plugin: String,
  pub label: Entity<TextInput>,
  pub fields: Vec<(String, String, bool, Entity<TextInput>)>,
  pub tools: Option<Result<Vec<mcp::Tool>, String>>,
}

fn state(m: &mut Market, id: &str, cx: &mut Context<Market>) -> State {
  let p = m.installed.iter().find(|p| p.id == id).cloned().unwrap_or_default();
  let fields: Vec<(String, String, bool, Entity<TextInput>)> = p.config()["fields"]
    .as_array()
    .cloned()
    .unwrap_or_default()
    .into_iter()
    .map(|f| {
      let name = f["name"].as_str().unwrap_or_default().to_string();
      let label = f["label"].as_str().unwrap_or_default().to_string();
      let secret = f["secret"].as_bool().unwrap_or(false);
      let input = cx.new(|cx| TextInput::new(cx).placeholder(label.clone()).password(secret).size(Size::Sm));
      (name, label, secret, input)
    })
    .collect();
  let label = cx.new(|cx| TextInput::new(cx).placeholder(t("Label, e.g. work or personal")).size(Size::Sm));
  State { plugin: id.to_string(), label, fields, tools: None }
}

fn load_tools(m: &mut Market, id: &str, cx: &mut Context<Market>) {
  let rt = m.rt.clone();
  let id = id.to_string();
  cx.spawn(async move |this, cx| {
    let r = crate::tk::run(async move { agent::api::connect::tools(&rt, &id).await }).await;
    let _ = this.update(cx, |this, cx| {
      if let Some(s) = &mut this.detail {
        s.tools = Some(r.map_err(|e| e.to_string()));
      }
      cx.notify();
    });
  })
  .detach();
}

pub fn render(m: &mut Market, id: &str, _window: &mut Window, cx: &mut Context<Market>) -> AnyElement {
  let ink = ink(cx);
  if m.detail.as_ref().map(|d| d.plugin.as_str()) != Some(id) {
    m.detail = Some(state(m, id, cx));
  }
  let Some(p) = m.installed.iter().find(|p| p.id == id).cloned() else {
    return div().child(t("Loading…")).into_any_element();
  };
  let accounts = m.accounts.get(id).cloned().unwrap_or_default();
  let multi = p.config()["multi_account"].as_bool().unwrap_or(true);
  let connected = p.status == "connected";
  if connected && m.detail.as_ref().is_some_and(|d| d.tools.is_none()) {
    if let Some(d) = &mut m.detail {
      d.tools = Some(Ok(Vec::new()));
    }
    load_tools(m, id, cx);
  }
  let desc = p.config()["description"].as_str().unwrap_or_default().to_string();
  let mut col = div()
    .flex()
    .flex_col()
    .gap(px(10.0))
    .child(div().flex().items_center().gap(px(8.0)).child(div().text_size(px(18.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(p.name.clone())).child(status_badge(&p.status)))
    .child(div().text_size(px(13.0)).text_color(ink.dimmed).child(SharedString::from(desc)));

  // Accounts.
  col = col.child(div().pt(px(6.0)).text_size(px(11.0)).font_weight(gpui::FontWeight::SEMIBOLD).text_color(ink.dimmed).child(t("ACCOUNTS")));
  for a in &accounts {
    let (pid, aid) = (p.id.clone(), a.id.clone());
    col = col.child(
      div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .p(px(8.0))
        .rounded(px(8.0))
        .bg(ink.surface)
        .child(div().flex_1().child(if a.label.is_empty() { t("Default").to_string() } else { a.label.clone() }))
        .child(Button::new(SharedString::from(format!("rma-{}", a.id)), t("Remove")).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(move |this, _, _, cx| {
          let rt = this.rt.clone();
          let (pid, aid) = (pid.clone(), aid.clone());
          this.run(cx, async move { agent::api::connect::remove_account(&rt, &pid, &aid).await }, |this, _, cx| this.load(cx));
        }))),
    );
  }
  let can_add = accounts.is_empty() || multi;
  if can_add && p.kind != "http" {
    let d = m.detail.as_ref().unwrap();
    let mut form = div().flex().flex_col().gap(px(6.0)).p(px(10.0)).rounded(px(8.0)).border_1().border_color(ink.border);
    if !accounts.is_empty() {
      form = form.child(div().font_weight(gpui::FontWeight::MEDIUM).child(t("Add Another Account")));
    }
    if multi {
      form = form.child(d.label.clone());
    }
    if p.kind == "oauth" {
      let waiting = p.status == "waiting for authorization";
      let pid = p.id.clone();
      form = form
        .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("Sign in in your browser. Tokens stay in your Keychain and never reach the model. Finish within 10 minutes.")))
        .child(
          Group::new().gap(Size::Xs).child(Button::new("authorize", if waiting { t("Reopen") } else if p.status == "needs auth" && !accounts.is_empty() { t("Retry") } else { t("Authorize") }).size(Size::Sm).on_click(cx.listener(move |this, _, _, cx| {
            let label = this.detail.as_ref().map(|d| d.label.read(cx).text()).unwrap_or_default();
            let rt = this.rt.clone();
            let pid = pid.clone();
            this.status = Some(t("Waiting for authorization…").into());
            this.run(cx, async move { agent::api::connect::authorize(&rt, &pid, &label).await }, |this, url, cx| {
              cx.open_url(&url);
              this.load(cx);
            });
          }))),
        );
    } else {
      for (_, label, _, input) in &d.fields {
        form = form.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(label.clone())).child(input.clone());
      }
      let pid = p.id.clone();
      form = form.child(Button::new("connect", t("Connect")).size(Size::Sm).on_click(cx.listener(move |this, _, _, cx| {
        let Some(d) = &this.detail else { return };
        let label = d.label.read(cx).text();
        let values: HashMap<String, String> = d.fields.iter().map(|(n, _, _, i)| (n.clone(), i.read(cx).text().trim().to_string())).collect();
        let rt = this.rt.clone();
        let pid = pid.clone();
        this.status = Some(t("Connecting…").into());
        this.run(cx, async move { agent::api::connect::connect_fields(&rt, &pid, &label, values).await }, |this, _, cx| {
          this.status = Some(t("Connected.").into());
          if let Some(d) = &mut this.detail {
            d.tools = None;
          }
          this.load(cx);
        });
      })));
    }
    col = col.child(form);
  }

  // Tools.
  if connected {
    col = col.child(div().pt(px(6.0)).text_size(px(11.0)).font_weight(gpui::FontWeight::SEMIBOLD).text_color(ink.dimmed).child(t("TOOLS")));
    match m.detail.as_ref().and_then(|d| d.tools.clone()) {
      Some(Ok(tools)) if tools.is_empty() => col = col.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("Asking the server for its tools…"))),
      Some(Ok(tools)) => {
        let off = p.disabled();
        for tool in tools {
          let on = !off.contains(&tool.name);
          let (pid, tn) = (p.id.clone(), tool.name.clone());
          col = col.child(
            div()
              .flex()
              .items_center()
              .gap(px(8.0))
              .py(px(4.0))
              .child(div().flex_1().min_w_0().flex().flex_col().child(div().text_size(px(13.0)).font_family("Menlo").child(tool.name.clone())).child(div().text_size(px(11.5)).text_color(ink.dimmed).truncate().child(SharedString::from(tool.description.clone()))))
              .child(Switch::new(SharedString::from(format!("tool-{}", tool.name))).checked(on).color(guise::ColorName::Violet).on_change(cx.listener(move |this, _, _, cx| {
                let rt = this.rt.clone();
                let (pid, tn) = (pid.clone(), tn.clone());
                this.run(cx, async move { agent::api::connect::toggle_tool(&rt, &pid, &tn, !on).await }, |this, _, cx| this.load(cx));
              }))),
          );
        }
      }
      Some(Err(e)) => col = col.child(div().text_size(px(12.0)).text_color(ink.danger).child(SharedString::from(e))),
      None => {}
    }
  }
  let pid = p.id.clone();
  col = col.child(div().pt(px(10.0)).child(Button::new("remove-plugin", t("Remove")).size(Size::Xs).variant(Variant::Light).color(guise::ColorName::Red).on_click(cx.listener(move |this, _, _, cx| {
    let rt = this.rt.clone();
    let pid = pid.clone();
    this.run(cx, async move { agent::api::connect::remove(&rt, &pid).await }, |this, _, cx| {
      this.open = None;
      this.detail = None;
      this.load(cx);
    });
  }))));
  col.into_any_element()
}
