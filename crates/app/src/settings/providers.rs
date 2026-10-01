//! Settings → Providers: where Bots' models come from. Profiles are HTTP
//! chat-completions endpoints (xAI, OpenAI, LiteLLM, Ollama local/cloud,
//! custom) or CLI agents (Claude Code, Codex). Keys are stored as pointers:
//! an environment variable, a Synapse secret, or the Keychain.

use super::{heading, save, Dialog};
use crate::i18n::t;
use crate::theme::ink;
use crate::tk;
use agent::Runtime;
use config::{Credential, Profile};
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, Entity, SharedString, Window};
use guise::{Badge, Button, Group, IconName, Select, Size, TextInput, Variant};
use std::collections::HashMap;

pub const CREDS: [&str; 4] = ["Environment variable", "Keychain", "Synapse secret", "No key"];

pub struct Form {
  pub preset: Entity<Select>,
  pub name: Entity<TextInput>,
  pub endpoint: Entity<TextInput>,
  pub cred: Entity<Select>,
  pub value: Entity<TextInput>,
  pub var: Entity<TextInput>,
  pub model: Entity<TextInput>,
  pub fast_model: Entity<TextInput>,
  pub found: HashMap<String, Result<Vec<String>, String>>,
  pub adding: bool,
}

impl Form {
  pub fn new(rt: &Runtime, _window: &mut Window, cx: &mut Context<Dialog>) -> Self {
    let s = rt.settings();
    Self {
      preset: cx.new(|cx| Select::new(cx).data(provider::preset::IDS.iter().map(|(_, l)| *l)).selected(3).size(Size::Sm)),
      name: cx.new(|cx| TextInput::new(cx).placeholder(t("Name, e.g. litellm")).size(Size::Sm)),
      endpoint: cx.new(|cx| TextInput::new(cx).placeholder("http://127.0.0.1:4000/v1").size(Size::Sm)),
      cred: cx.new(|cx| Select::new(cx).data(CREDS.iter().map(|c| t(c))).selected(0).size(Size::Sm)),
      value: cx.new(|cx| TextInput::new(cx).placeholder(t("LITELLM_API_KEY")).size(Size::Sm)),
      var: cx.new(|cx| TextInput::new(cx).placeholder(t("Variable it exports, e.g. OPENROUTER_API_KEY")).size(Size::Sm)),
      model: cx.new(|cx| TextInput::new(cx).value(&s.model).placeholder(t("Model")).size(Size::Sm)),
      fast_model: cx.new(|cx| TextInput::new(cx).value(&s.fast_model).placeholder(t("Fast model (optional)")).size(Size::Sm)),
      found: HashMap::new(),
      adding: false,
    }
  }
}

fn set_profiles(rt: &Runtime, f: impl FnOnce(&mut Vec<Profile>)) {
  let mut list = rt.profiles();
  f(&mut list);
  save(rt, |s| s.providers = list);
}

fn discover(d: &mut Dialog, p: Profile, cx: &mut Context<Dialog>) {
  let name = p.name.clone();
  d.providers.found.insert(name.clone(), Ok(Vec::new()));
  let rt = d.rt.clone();
  cx.spawn(async move |this, cx| {
    let r = tk::run(async move { provider::discover(&p).await }).await;
    let _ = this.update(cx, |this, cx| {
      match r {
        Ok(models) => {
          let keep = models.clone();
          let n = name.clone();
          set_profiles(&rt, |list| {
            if let Some(x) = list.iter_mut().find(|x| x.name == n) {
              x.models = keep;
            }
          });
          this.providers.found.insert(name, Ok(models));
        }
        Err(e) => {
          this.providers.found.insert(name, Err(e.to_string()));
        }
      }
      cx.notify();
    });
  })
  .detach();
}

fn add(d: &mut Dialog, cx: &mut Context<Dialog>) {
  let f = &d.providers;
  let idx = f.preset.read(cx).selected_index().unwrap_or(3);
  let (preset, label) = provider::preset::IDS[idx];
  let mut name = f.name.read(cx).text().trim().to_lowercase().replace(' ', "");
  if name.is_empty() {
    name = preset.replace('-', "");
  }
  if d.rt.profiles().iter().any(|p| p.name == name) {
    d.status = Some(crate::i18n::tf("A provider named {} already exists.", &[&name]));
    cx.notify();
    return;
  }
  let mut p = provider::preset::build(preset, &name);
  let endpoint = f.endpoint.read(cx).text();
  if !endpoint.trim().is_empty() && !p.is_process() {
    p.endpoint = endpoint.trim().trim_end_matches('/').to_string();
  }
  if !p.is_process() && preset != "xai" {
    let value = f.value.read(cx).text().trim().to_string();
    p.credential = match f.cred.read(cx).selected_index().unwrap_or(0) {
      0 if !value.is_empty() => Credential::Env { var: value },
      0 => p.credential.clone(),
      1 => {
        let account = provider::credential::account(&name);
        if !value.is_empty() {
          let _ = config::secret::set(&account, &value);
        }
        Credential::Keychain { account }
      }
      2 => Credential::Synapse { secret: value, var: f.var.read(cx).text().trim().to_string() },
      _ => Credential::None,
    };
  }
  if p.is_process() {
    if let Err(e) = provider::process::check_command(&p.command) {
      d.status = Some(e.to_string());
      cx.notify();
      return;
    }
  }
  let first = d.rt.settings().providers.is_empty();
  let added = p.clone();
  set_profiles(&d.rt, |list| {
    if first && !list.iter().any(|x| x.preset == "xai") {
      list.clear();
    }
    list.push(p);
  });
  if first {
    let n = added.name.clone();
    save(&d.rt, |s| s.provider = n);
  }
  d.status = Some(crate::i18n::tf("Added {}.", &[label]));
  d.providers.adding = false;
  for e in [&d.providers.name, &d.providers.endpoint, &d.providers.value, &d.providers.var] {
    e.update(cx, |i, cx| i.set_text("", cx));
  }
  discover(d, added, cx);
}

pub fn render(d: &mut Dialog, _window: &mut Window, cx: &mut Context<Dialog>) -> AnyElement {
  let ink = ink(cx);
  let s = d.rt.settings();
  let profiles = d.rt.profiles();
  let default = d.rt.profile("").map(|p| p.name).unwrap_or_default();
  let mut col = div().flex().flex_col().gap(px(8.0));
  col = col.child(heading(t("Providers"), cx)).child(
    div().text_size(px(12.5)).text_color(ink.dimmed).child(t(
      "Bots talk to models through these. HTTP providers (xAI, OpenAI, LiteLLM, Ollama) get Asylum's tools; CLI agents (Claude Code, Codex) run with their own tools in the computer's workspace.",
    )),
  );
  for p in profiles.clone() {
    let is_default = p.name == default;
    let is_fast = s.fast_provider == p.name;
    let (n1, n2, n3, n4) = (p.name.clone(), p.name.clone(), p.name.clone(), p.name.clone());
    let p2 = p.clone();
    let where_ = if p.is_process() { format!("{} {}", p.command, p.args.join(" ")) } else { p.endpoint.clone() };
    let label = provider::preset::IDS.iter().find(|(id, _)| *id == p.preset).map(|(_, l)| *l).unwrap_or("Custom");
    let mut card = div()
      .flex()
      .flex_col()
      .gap(px(6.0))
      .p(px(12.0))
      .rounded(px(10.0))
      .border_1()
      .border_color(if is_default { ink.primary } else { ink.border })
      .child(
        div()
          .flex()
          .items_center()
          .gap(px(8.0))
          .child(div().text_color(ink.dimmed).child(guise::Icon::new(if p.is_process() { IconName::Terminal } else { IconName::Globe }).size(Size::Sm)))
          .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(p.name.clone()))
          .child(Badge::new(t(label)).size(Size::Xs).variant(Variant::Light))
          .when(is_default, |d| d.child(Badge::new(t("Default")).size(Size::Xs).color(guise::ColorName::Violet)))
          .when(is_fast, |d| d.child(Badge::new(t("Fast")).size(Size::Xs).color(guise::ColorName::Teal)))
          .child(div().flex_1())
          .child(Group::new().gap(Size::Xs)
            .when(!is_default, |g| g.child(Button::new(SharedString::from(format!("use-{n1}")), t("Use as default")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| {
              let n = n1.clone();
              save(&this.rt, |s| { s.provider = n; s.model = String::new(); });
              this.providers.model.update(cx, |i, cx| i.set_text("", cx));
              cx.notify();
            }))))
            .child(Button::new(SharedString::from(format!("fast-{n4}")), if is_fast { t("Unset fast") } else { t("Use for quick work") }).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(move |this, _, _, cx| {
              let n = if is_fast { String::new() } else { n4.clone() };
              save(&this.rt, |s| s.fast_provider = n);
              cx.notify();
            })))
            .child(Button::new(SharedString::from(format!("models-{n2}")), t("Refresh models")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(move |this, _, _, cx| discover(this, p2.clone(), cx))))
            .when(profiles.len() > 1, |g| g.child(Button::new(SharedString::from(format!("rm-{n3}")), t("Remove")).size(Size::Xs).variant(Variant::Subtle).color(guise::ColorName::Red).on_click(cx.listener(move |this, _, _, cx| {
              let n = n3.clone();
              set_profiles(&this.rt, |list| list.retain(|x| x.name != n));
              if this.rt.settings().provider == n3 {
                save(&this.rt, |s| s.provider = String::new());
              }
              cx.notify();
            }))))),
      )
      .child(div().text_size(px(12.0)).text_color(ink.dimmed).font_family("Menlo").truncate().child(where_))
      .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(crate::i18n::tf("Key: {}", &[&provider::credential::describe(&p.credential)])));
    match d.providers.found.get(&p.name) {
      Some(Err(e)) => card = card.child(div().text_size(px(12.0)).text_color(ink.danger).child(SharedString::from(e.clone()))),
      Some(Ok(v)) if v.is_empty() => card = card.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("Asking for models…"))),
      _ => {}
    }
    if !p.models.is_empty() {
      let mut chips = div().flex().flex_wrap().gap(px(4.0));
      let current = if is_default { s.model.clone() } else { String::new() };
      for m in p.models.iter().take(40) {
        let (m2, pn) = (m.clone(), p.name.clone());
        let on = &current == m;
        chips = chips.child(
          div()
            .id(SharedString::from(format!("chip-{}-{m}", p.name)))
            .px(px(8.0))
            .py(px(2.0))
            .rounded_full()
            .text_size(px(11.5))
            .border_1()
            .border_color(if on { ink.primary } else { ink.border })
            .bg(if on { ink.primary.opacity(0.15) } else { ink.body })
            .cursor_pointer()
            .child(m.clone())
            .on_click(cx.listener(move |this, _, _, cx| {
              let (m, pn) = (m2.clone(), pn.clone());
              save(&this.rt, |s| {
                s.provider = pn;
                s.model = m.clone();
              });
              this.providers.model.update(cx, |i, cx| i.set_text(&m, cx));
              cx.notify();
            })),
        );
      }
      card = card.child(chips);
    }
    col = col.child(card);
  }

  col = col.child(heading(t("Default model"), cx)).child(
    div()
      .flex()
      .gap(px(8.0))
      .items_center()
      .child(div().w(px(420.0)).child(d.providers.model.clone()))
      .child(Button::new("save-model", t("Save")).size(Size::Xs).on_click(cx.listener(|this, _, _, cx| {
        let m = this.providers.model.read(cx).text().trim().to_string();
        save(&this.rt, |s| s.model = m);
        this.status = Some(t("Saved.").into());
        cx.notify();
      }))),
  );
  col = col.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("Leave empty on xAI to use the best Grok model your key can use. Any Bot can pin its own provider and model in Bot settings.")));
  col = col.child(
    div()
      .flex()
      .gap(px(8.0))
      .items_center()
      .child(div().w(px(420.0)).child(d.providers.fast_model.clone()))
      .child(Button::new("save-fast", t("Save")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, _, cx| {
        let m = this.providers.fast_model.read(cx).text().trim().to_string();
        save(&this.rt, |s| s.fast_model = m);
        cx.notify();
      }))),
  );

  col = col.child(heading(t("Add a provider"), cx));
  if !d.providers.adding {
    col = col.child(Button::new("add-provider", t("Add provider")).size(Size::Sm).left_section(guise::Icon::new(IconName::Plus).size(Size::Xs)).on_click(cx.listener(|this, _, _, cx| {
      this.providers.adding = true;
      cx.notify();
    })));
  } else {
    let f = &d.providers;
    let idx = f.preset.read(cx).selected_index().unwrap_or(3);
    let preset = provider::preset::IDS[idx].0;
    let process = matches!(preset, "claude-code" | "codex");
    let cred = f.cred.read(cx).selected_index().unwrap_or(0);
    let hint = match preset {
      "lite-llm" => t("A LiteLLM proxy serves every model it fronts behind one endpoint; its model_list is the source of truth."),
      "ollama" => t("Ollama running on this Mac. No key needed."),
      "ollama-cloud" => t("Ollama Cloud's OpenAI-compatible endpoint, keyed by OLLAMA_API_KEY."),
      "claude-code" => t("Runs `claude -p` with streaming JSON. Claude uses its own tools in the workspace."),
      "codex" => t("Runs `codex exec` in the workspace with its own tools."),
      "openai" => t("OpenAI's API (the models behind ChatGPT), keyed by OPENAI_API_KEY."),
      "anthropic" => t("Claude models (Opus, Sonnet, Haiku) through Anthropic's OpenAI-compatible API, keyed by ANTHROPIC_API_KEY."),
      "xai" => t("xAI's Grok API. Uses the key saved under xAI below."),
      _ => t("Any OpenAI-compatible chat-completions endpoint."),
    };
    let mut form = div().flex().flex_col().gap(px(8.0)).p(px(12.0)).rounded(px(10.0)).bg(ink.surface)
      .child(f.preset.clone())
      .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(hint))
      .child(f.name.clone());
    if !process && preset != "xai" {
      form = form.child(f.endpoint.clone()).child(f.cred.clone());
      if cred < 3 {
        form = form.child(f.value.clone());
      }
      if cred == 2 {
        form = form.child(f.var.clone());
      }
      let explain = match cred {
        0 => t("Asylum stores the variable's name, never its value."),
        1 => t("Paste the key; it goes to the macOS Keychain."),
        2 => t("A secret in Synapse's vault, like apis.OpenRouter, and the variable it exports."),
        _ => t("No key is sent."),
      };
      form = form.child(div().text_size(px(11.5)).text_color(ink.dimmed).child(explain));
    }
    form = form.child(
      Group::new()
        .gap(Size::Sm)
        .child(Button::new("confirm-add", t("Add")).size(Size::Sm).on_click(cx.listener(|this, _, _, cx| add(this, cx))))
        .child(Button::new("cancel-add", t("Cancel")).size(Size::Sm).variant(Variant::Default).on_click(cx.listener(|this, _, _, cx| {
          this.providers.adding = false;
          cx.notify();
        }))),
    );
    col = col.child(form);
  }

  let voice = d.rt.settings().voice_enabled;
  col = col.child(heading(if voice { t("xAI (voice and images)") } else { t("xAI (images)") }, cx)).child(super::row(
    t("xAI API key"),
    Some(match (d.has_key, voice) {
      (true, true) => t("Saved in the Keychain. Used for dictation, voice chat, voice memos, and images."),
      (false, true) => t("Needed for dictation, voice chat, voice memos, and image generation."),
      (true, false) => t("Saved in the Keychain. Used for image generation."),
      (false, false) => t("Needed for image generation."),
    }),
    div().w(px(260.0)).child(d.key.clone()),
    cx,
  ));
  if let Some(st) = &d.status {
    col = col.child(div().text_size(px(12.0)).text_color(ink.primary).child(SharedString::from(st.clone())));
  }
  col.into_any_element()
}
