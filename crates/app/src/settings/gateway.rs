//! Per-profile controls under an HTTP provider's card: the key (kept in the
//! Keychain; an exported variable still wins), LiteLLM teams with their
//! model scope, reasoning effort, the fallback chain, and a real check.

use super::{save, Dialog};
use crate::i18n::{t, tf};
use crate::theme::ink;
use config::{Credential, Profile};
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString};
use guise::{Button, Size, TextInput, Variant};
use provider::litellm::{Reach, Team};

pub const EFFORTS: [(&str, &str); 4] = [("", "Off"), ("low", "Low"), ("medium", "Medium"), ("high", "High")];

/// What this page has learned about each profile while it's open.
#[derive(Default)]
pub struct State {
  pub key: std::collections::HashMap<String, gpui::Entity<TextInput>>,
  pub teams: std::collections::HashMap<String, Result<Vec<Team>, String>>,
  pub reach: std::collections::HashMap<String, Reach>,
  pub check: std::collections::HashMap<String, Result<String, String>>,
  pub busy: std::collections::HashSet<String>,
}

fn update(rt: &agent::Runtime, name: &str, f: impl FnOnce(&mut Profile)) {
  let mut list = rt.profiles();
  if let Some(p) = list.iter_mut().find(|p| p.name == name) {
    f(p);
  }
  save(rt, |s| s.providers = list);
}

/// Where a pasted key goes for this profile: its Keychain entry, plus the
/// environment variable that overrides it, if the profile names one.
fn key_target(p: &Profile) -> Option<(String, Option<String>)> {
  match &p.credential {
    Credential::Env { var } => Some((provider::credential::account(&p.name), Some(var.clone()))),
    Credential::Keychain { account } => Some((account.clone(), None)),
    // Synapse or no key: a pasted key goes to the Keychain and replaces it.
    _ if p.preset != "ollama" => Some((provider::credential::account(&p.name), None)),
    _ => None,
  }
}

fn chip(id: String, label: String, on: bool, cx: &Context<Dialog>) -> gpui::Stateful<gpui::Div> {
  let ink = ink(cx);
  div()
    .id(SharedString::from(id))
    .px(px(8.0))
    .py(px(2.0))
    .rounded_full()
    .text_size(px(11.5))
    .border_1()
    .border_color(if on { ink.primary } else { ink.border })
    .bg(if on { ink.primary.opacity(0.15) } else { ink.body })
    .cursor_pointer()
    .child(label)
}

pub fn render(d: &mut Dialog, p: &Profile, cx: &mut Context<Dialog>) -> AnyElement {
  let ink = ink(cx);
  let name = p.name.clone();
  let litellm = p.preset == "lite-llm";
  let mut col = div().flex().flex_col().gap(px(6.0)).pt(px(4.0));

  // The key: typed here, checked against the gateway, kept in the Keychain.
  if let Some((account, var)) = key_target(p) {
    let var = var.unwrap_or_default();
    let exported = !var.is_empty() && std::env::var(&var).is_ok_and(|v| !v.trim().is_empty());
    let stored = config::secret::get(&account).is_some();
    let input = d.gateway.key.entry(name.clone()).or_insert_with(|| cx.new(|cx| TextInput::new(cx).password(true).placeholder(t("Paste an API key")).size(Size::Sm))).clone();
    let n = name.clone();
    let status = match (exported, stored) {
      _ if matches!(p.credential, Credential::Synapse { .. }) => t("Using a Synapse secret. Paste a key to use it instead.").to_string(),
      _ if matches!(p.credential, Credential::None) => t("No key. Paste one if this provider needs it.").to_string(),
      (true, _) => tf("Using {} from the environment.", &[&var]),
      (false, true) => t("Key saved in the Keychain.").to_string(),
      (false, false) if var.is_empty() => t("No key saved yet: paste one.").to_string(),
      (false, false) => tf("No key yet: paste one, or export {}.", &[&var]),
    };
    col = col.child(
      div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(div().w(px(240.0)).child(input))
        .child(Button::new(SharedString::from(format!("savekey-{n}")), t("Save key")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| save_key(this, &n, cx))))
        .child(div().text_size(px(11.5)).text_color(ink.dimmed).child(status)),
    );
  }

  // LiteLLM teams: pick one and the model list follows its scope.
  if litellm {
    let n = name.clone();
    let mut row = div().flex().flex_wrap().items_center().gap(px(4.0)).child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("Team")));
    match d.gateway.teams.get(&name) {
      Some(Ok(teams)) if teams.is_empty() => row = row.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("This key isn't in a team; it sees the gateway's whole catalog."))),
      Some(Ok(teams)) => {
        for team in teams.clone() {
          let on = p.team.as_deref() == Some(team.id.as_str());
          let (n2, tm) = (name.clone(), team.clone());
          let label = if team.models.is_empty() { team.name.clone() } else { format!("{} ({})", team.name, team.models.len()) };
          row = row.child(chip(format!("team-{name}-{}", team.id), label, on, cx).on_click(cx.listener(move |this, _, _, cx| pick_team(this, &n2, if on { None } else { Some(tm.clone()) }, cx))));
        }
      }
      Some(Err(e)) => row = row.child(div().text_size(px(12.0)).text_color(ink.danger).child(SharedString::from(e.clone()))),
      None => {}
    }
    row = row.child(Button::new(SharedString::from(format!("teams-{n}")), if d.gateway.busy.contains(&name) { t("Loading…") } else { t("Load teams") }).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(move |this, _, _, cx| load_teams(this, &n, cx))));
    col = col.child(row);
    if let Some(r) = d.gateway.reach.get(&name) {
      let missing = r.missing();
      if !missing.is_empty() {
        col = col.child(div().text_size(px(12.0)).text_color(ink.warning).child(tf("This key can't call {} even though its team allows them. Ask your gateway admin to widen the key.", &[&missing.join(", ")])));
      }
    }
  }

  // Reasoning effort, sent with every request to this provider.
  let mut effort = div().flex().items_center().gap(px(4.0)).child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("Reasoning")));
  for (value, label) in EFFORTS {
    let on = p.reasoning == value;
    let n = name.clone();
    effort = effort.child(chip(format!("eff-{name}-{label}"), t(label).to_string(), on, cx).on_click(cx.listener(move |this, _, _, cx| {
      update(&this.rt, &n, |p| p.reasoning = value.to_string());
      cx.notify();
    })));
  }
  col = col.child(effort);

  // Fallback chain and the real check.
  let fallbacks = d.rt.settings().fallback_providers;
  let position = fallbacks.iter().position(|f| *f == name);
  let (n1, n2) = (name.clone(), name.clone());
  let mut actions = div()
    .flex()
    .items_center()
    .gap(px(6.0))
    .child(Button::new(SharedString::from(format!("fb-{name}")), if position.is_some() { t("Remove from fallbacks") } else { t("Use as fallback") }).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(move |this, _, _, cx| {
      let n = n1.clone();
      save(&this.rt, |s| {
        if s.fallback_providers.contains(&n) {
          s.fallback_providers.retain(|f| *f != n);
        } else {
          s.fallback_providers.push(n);
        }
      });
      cx.notify();
    })))
    .child(Button::new(SharedString::from(format!("check-{name}")), t("Check provider")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(move |this, _, _, cx| check(this, &n2, cx))));
  if let Some(i) = position {
    actions = actions.child(div().text_size(px(11.5)).text_color(ink.dimmed).child(tf("Fallback {}", &[&(i + 1).to_string()])));
  }
  col = col.child(actions);
  match d.gateway.check.get(&name) {
    Some(Ok(reply)) => col = col.child(div().text_size(px(12.0)).text_color(ink.success).child(tf("Answered: “{}”", &[reply]))),
    Some(Err(e)) => col = col.child(div().text_size(px(12.0)).text_color(ink.danger).child(SharedString::from(e.clone()))),
    None => {}
  }
  col.into_any_element()
}

fn save_key(d: &mut Dialog, name: &str, cx: &mut Context<Dialog>) {
  let Some(input) = d.gateway.key.get(name).cloned() else { return };
  let key = input.read(cx).text().trim().to_string();
  if key.is_empty() {
    return;
  }
  let Some(p) = d.rt.profile(name) else { return };
  let n = name.to_string();
  d.status = Some(t("Checking…").into());
  cx.spawn(async move |this, cx| {
    // Refused keys are refused now, not at an Agent's first turn.
    let r = crate::tk::run(async move {
      if p.preset == "lite-llm" {
        provider::litellm::check(&p.endpoint, &key).await?;
      } else {
        ::chat::Client::new(key.clone(), Some(p.endpoint.clone())).retries(1).models().await.map_err(|e| anyhow::anyhow!("That key didn't work: {e}"))?;
      }
      let (account, _) = key_target(&p).ok_or_else(|| anyhow::anyhow!("This provider doesn't take a key."))?;
      config::secret::set(&account, &key)?;
      // A Synapse or keyless profile now reads the pasted key.
      anyhow::Ok(matches!(p.credential, Credential::Synapse { .. } | Credential::None).then_some(account))
    })
    .await;
    let _ = this.update(cx, |this, cx| {
      this.status = Some(match r {
        Ok(switch) => {
          if let Some(account) = switch {
            update(&this.rt, &n, |p| p.credential = Credential::Keychain { account });
          }
          input.update(cx, |i, cx| i.set_text("", cx));
          t("Key saved.").into()
        }
        Err(e) => e.to_string(),
      });
      if this.rt.profile(&n).is_some_and(|p| p.preset == "lite-llm") {
        load_teams(this, &n, cx);
      }
      cx.notify();
    });
  })
  .detach();
}

fn load_teams(d: &mut Dialog, name: &str, cx: &mut Context<Dialog>) {
  let Some(p) = d.rt.profile(name) else { return };
  let n = name.to_string();
  d.gateway.busy.insert(n.clone());
  cx.spawn(async move |this, cx| {
    let r = crate::tk::run(async move {
      let key = provider::credential::resolve_for(&p).await?.unwrap_or_default();
      let teams = provider::litellm::teams(&p.endpoint, &key).await?;
      let current = teams.iter().find(|t| Some(&t.id) == p.team.as_ref()).cloned();
      let reach = provider::litellm::reach(&p.endpoint, &key, current.as_ref()).await;
      anyhow::Ok((teams, reach))
    })
    .await;
    let _ = this.update(cx, |this, cx| {
      this.gateway.busy.remove(&n);
      match r {
        Ok((teams, reach)) => {
          this.gateway.teams.insert(n.clone(), Ok(teams));
          this.gateway.reach.insert(n, reach);
        }
        Err(e) => {
          this.gateway.teams.insert(n, Err(e.to_string()));
        }
      }
      cx.notify();
    });
  })
  .detach();
}

/// Scope the profile to a team (or clear it) and refresh its models.
fn pick_team(d: &mut Dialog, name: &str, team: Option<Team>, cx: &mut Context<Dialog>) {
  let (id, label) = team.as_ref().map(|t| (Some(t.id.clone()), t.name.clone())).unwrap_or((None, String::new()));
  update(&d.rt, name, |p| {
    p.team = id;
    p.team_name = label;
  });
  if let Some(p) = d.rt.profile(name) {
    super::providers::discover(d, p, cx);
  }
  load_teams(d, name, cx);
}

fn check(d: &mut Dialog, name: &str, cx: &mut Context<Dialog>) {
  let Some(p) = d.rt.profile(name) else { return };
  let s = d.rt.settings();
  let model = if !p.model.is_empty() {
    p.model.clone()
  } else if s.provider == p.name && !s.model.is_empty() {
    s.model.clone()
  } else {
    p.models.first().cloned().unwrap_or_default()
  };
  let n = name.to_string();
  if model.is_empty() {
    d.gateway.check.insert(n, Err(t("Pick a model first.").into()));
    cx.notify();
    return;
  }
  let ws = d.rt.computer.workspace();
  d.gateway.check.remove(&n);
  d.status = Some(tf("Asking {}…", &[&model]));
  cx.spawn(async move |this, cx| {
    let r = crate::tk::run(async move { provider::check(&p, &model, &ws).await }).await;
    let _ = this.update(cx, |this, cx| {
      this.status = None;
      this.gateway.check.insert(n, r.map_err(|e| e.to_string()));
      cx.notify();
    });
  })
  .detach();
}
