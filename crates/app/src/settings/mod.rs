//! The settings dialog (⌘,): General, Computer, Usage, Team
//! Setup, and Updates. Changes save immediately to bots.json; secrets go to
//! the keychain.

pub mod about;
pub mod activity;
pub mod computer;
pub mod general;
pub mod providers;
pub mod required;
pub mod rules;
pub mod updates;
pub mod usage;

use crate::i18n::t;
use crate::root::Root;
use crate::theme::ink;
use agent::Runtime;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, App, Context, Entity, SharedString, Subscription, WeakEntity, Window};
use guise::{IconName, Select, SelectEvent, Size, TextInput, TextInputEvent};

pub use about::about;

pub const PAGES: [(&str, &str, IconName); 7] = [
  ("general", "General", IconName::Settings),
  ("providers", "Providers", IconName::Cpu),
  ("computer", "Computer", IconName::Monitor),
  ("usage", "Usage", IconName::ChartColumn),
  ("team", "Team Setup", IconName::Users),
  ("activity", "Activity", IconName::Activity),
  ("updates", "Updates", IconName::RefreshCw),
];

pub struct Dialog {
  pub rt: Runtime,
  pub root: WeakEntity<Root>,
  pub page: &'static str,
  pub key: Entity<TextInput>,
  pub name: Entity<TextInput>,
  pub providers: providers::Form,
  pub weekly: Entity<TextInput>,
  pub monthly: Entity<TextInput>,
  pub rule: Entity<TextInput>,
  pub theme: Entity<Select>,
  pub language: Entity<Select>,
  pub zone: Entity<Select>,
  pub local: Entity<Select>,
  pub mic: Entity<Select>,
  pub voice: Entity<Select>,
  pub rules: Vec<store::Rule>,
  pub has_key: bool,
  pub status: Option<String>,
  pub disk: Option<agent::api::computer::Disk>,
  pub backups: usize,
  pub usage: agent::usage::Summary,
  pub by_bot: Vec<(String, i64)>,
  pub update: Option<String>,
  pub links: Vec<store::slack::Link>,
  pub activity: Option<activity::Data>,
  pub update_at: Option<i64>,
  pub otel: Entity<TextInput>,
  pub link_user: Entity<TextInput>,
  pub link_name: Entity<TextInput>,
  pub _subs: Vec<Subscription>,
}

pub fn zones() -> Vec<String> {
  let mut v = vec![t("Auto-detect").to_string()];
  v.extend(chrono_tz::TZ_VARIANTS.iter().map(|z| z.name().to_string()));
  v
}

pub const VOICES: [&str; 5] = ["eve", "ara", "rex", "sal", "leo"];

pub fn open(root: &mut Root, page: Option<&'static str>, window: &mut Window, cx: &mut Context<Root>) {
  let weak = cx.entity().downgrade();
  let rt = root.rt.clone();
  let view = cx.new(|cx| Dialog::new(rt, weak, page.unwrap_or("general"), window, cx));
  root.set_modal(view, cx);
}

/// Change settings and write them to bots.json.
pub fn save(rt: &Runtime, f: impl FnOnce(&mut config::Settings)) {
  let before = rt.settings();
  let mut s = before.clone();
  f(&mut s);
  let _ = config::save(&config::settings_path(), &s);
  let changed = changed_keys(&before, &s);
  rt.set_settings(s);
  if !changed.is_empty() {
    let rt = rt.clone();
    crate::tk::spawn(async move { agent::audit::change(&rt, "user", "settings.changed", &changed.join(", "), "").await });
  }
}

/// Which settings differ (names only).
fn changed_keys(a: &config::Settings, b: &config::Settings) -> Vec<String> {
  let (Ok(serde_json::Value::Object(a)), Ok(serde_json::Value::Object(b))) = (serde_json::to_value(a), serde_json::to_value(b)) else { return Vec::new() };
  a.iter().filter(|(k, v)| b.get(*k) != Some(*v)).map(|(k, _)| k.clone()).collect()
}

impl Dialog {
  fn new(rt: Runtime, root: WeakEntity<Root>, page: &'static str, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let s = rt.settings();
    let has_key = config::secret::xai_key().is_some();
    let key = cx.new(|cx| TextInput::new(cx).password(true).placeholder(if has_key { t("•••••••• saved in Keychain") } else { t("xai-…") }));
    let name = cx.new(|cx| TextInput::new(cx).value(&s.user_name).placeholder(t("Your name")));
    let providers = providers::Form::new(&rt, window, cx);
    let weekly = cx.new(|cx| TextInput::new(cx).value(&s.weekly_limit.to_string()));
    let monthly = cx.new(|cx| TextInput::new(cx).value(&s.monthly_limit.to_string()));
    let rule = cx.new(|cx| TextInput::new(cx).placeholder(t("e.g. Ask before sending any email to a customer")));
    let themes = [t("Follow System"), t("Light"), t("Dark")];
    let tidx = match s.appearance {
      config::Appearance::System => 0,
      config::Appearance::Light => 1,
      config::Appearance::Dark => 2,
    };
    let theme = cx.new(|cx| Select::new(cx).data(themes).selected(tidx).size(Size::Sm));
    let mut langs = vec![t("Follow System").to_string()];
    langs.extend(crate::i18n::LANGUAGES.iter().map(|(_, native, en)| if native == en { native.to_string() } else { format!("{native} — {en}") }));
    let lidx = if s.language == "system" { 0 } else { crate::i18n::LANGUAGES.iter().position(|(c, _, _)| *c == s.language).map(|i| i + 1).unwrap_or(0) };
    let language = cx.new(|cx| Select::new(cx).data(langs).selected(lidx).size(Size::Sm));
    let zs = zones();
    let zidx = if s.timezone == "auto" { 0 } else { zs.iter().position(|z| *z == s.timezone).unwrap_or(0) };
    let zone = cx.new(|cx| Select::new(cx).data(zs).selected(zidx).size(Size::Sm));
    let lopts = [t("Ask every time"), t("Always allow"), t("Never allow")];
    let lsel = match s.local_exec {
      config::LocalExec::Ask => 0,
      config::LocalExec::Always => 1,
      config::LocalExec::Never => 2,
    };
    let local = cx.new(|cx| Select::new(cx).data(lopts).selected(lsel).size(Size::Sm));
    let mut mics = vec![t("System default").to_string()];
    mics.extend(voice::mic::devices());
    let midx = mics.iter().position(|m| *m == s.microphone).unwrap_or(0);
    let mic = cx.new(|cx| Select::new(cx).data(mics).selected(midx).size(Size::Sm));
    let vidx = VOICES.iter().position(|v| v.eq_ignore_ascii_case(&s.voice)).unwrap_or(0);
    let voice = cx.new(|cx| Select::new(cx).data(VOICES.iter().map(|v| capital(v))).selected(vidx).size(Size::Sm));

    let mut subs = Vec::new();
    subs.push(cx.subscribe(&key, |this: &mut Dialog, _, ev: &TextInputEvent, cx| {
      if let TextInputEvent::Submit(k) = ev {
        this.save_key(k.clone(), cx);
      }
    }));
    let rt2 = rt.clone();
    subs.push(cx.subscribe(&name, move |_, _, ev: &TextInputEvent, _| {
      if let TextInputEvent::Change(n) = ev {
        let n = n.clone();
        save(&rt2, |s| s.user_name = n);
      }
    }));
    let rt2 = rt.clone();
    subs.push(cx.subscribe(&weekly, move |_, _, ev: &TextInputEvent, _| {
      if let TextInputEvent::Change(v) = ev {
        if let Ok(n) = v.replace([',', '_'], "").trim().parse::<u64>() {
          save(&rt2, |s| s.weekly_limit = n);
        }
      }
    }));
    let rt2 = rt.clone();
    subs.push(cx.subscribe(&monthly, move |_, _, ev: &TextInputEvent, _| {
      if let TextInputEvent::Change(v) = ev {
        if let Ok(n) = v.replace([',', '_'], "").trim().parse::<u64>() {
          save(&rt2, |s| s.monthly_limit = n);
        }
      }
    }));
    subs.push(cx.subscribe(&rule, |this: &mut Dialog, _, ev: &TextInputEvent, cx| {
      if let TextInputEvent::Submit(text) = ev {
        rules::add(this, "ask", text.clone(), cx);
      }
    }));
    let rt2 = rt.clone();
    subs.push(cx.subscribe_in(&theme, window, move |_, _, ev: &SelectEvent, w, cx| {
      let a = [config::Appearance::System, config::Appearance::Light, config::Appearance::Dark][ev.0.min(2)];
      save(&rt2, |s| s.appearance = a);
      crate::theme::install(crate::theme::dark_for(a, w.appearance()), cx);
      cx.refresh_windows();
    }));
    let rt2 = rt.clone();
    subs.push(cx.subscribe(&language, move |_, _, ev: &SelectEvent, cx| {
      let code = if ev.0 == 0 { "system".to_string() } else { crate::i18n::LANGUAGES[ev.0 - 1].0.to_string() };
      crate::i18n::set(&code);
      save(&rt2, |s| s.language = code);
      crate::menus::set(cx);
      cx.refresh_windows();
    }));
    let rt2 = rt.clone();
    subs.push(cx.subscribe(&zone, move |_, _, ev: &SelectEvent, _| {
      let z = if ev.0 == 0 { "auto".to_string() } else { zones()[ev.0].clone() };
      save(&rt2, |s| s.timezone = z);
    }));
    let rt2 = rt.clone();
    subs.push(cx.subscribe(&local, move |_, _, ev: &SelectEvent, _| {
      let v = [config::LocalExec::Ask, config::LocalExec::Always, config::LocalExec::Never][ev.0.min(2)];
      save(&rt2, |s| s.local_exec = v);
    }));
    let rt2 = rt.clone();
    subs.push(cx.subscribe(&mic, move |this: &mut Dialog, _, ev: &SelectEvent, cx| {
      let name = if ev.0 == 0 { String::new() } else { this.mic.read(cx).selected_value().map(|s| s.to_string()).unwrap_or_default() };
      save(&rt2, |s| s.microphone = name);
    }));
    let rt2 = rt.clone();
    subs.push(cx.subscribe(&voice, move |_, _, ev: &SelectEvent, _| {
      let v = VOICES[ev.0.min(VOICES.len() - 1)].to_string();
      save(&rt2, |s| s.voice = v);
    }));

    let mut d = Self {
      rt,
      root,
      page,
      key,
      name,
      providers,
      weekly,
      monthly,
      rule,
      theme,
      language,
      zone,
      local,
      mic,
      voice,
      rules: Vec::new(),
      has_key,
      status: None,
      disk: None,
      backups: 0,
      usage: Default::default(),
      by_bot: Vec::new(),
      update: None,
      links: Vec::new(),
      activity: None,
      update_at: None,
      otel: cx.new(|cx| TextInput::new(cx).value(&s.otel_endpoint).placeholder("http://127.0.0.1:4318").size(Size::Sm)),
      link_user: cx.new(|cx| TextInput::new(cx).placeholder(t("Slack member ID, e.g. U012AB3CD")).size(Size::Sm)),
      link_name: cx.new(|cx| TextInput::new(cx).placeholder(t("Name")).size(Size::Sm)),
      _subs: subs,
    };
    d.refresh(cx);
    d
  }

  pub fn refresh(&mut self, cx: &mut Context<Self>) {
    let rt = self.rt.clone();
    cx.spawn(async move |this, cx| {
      let r = crate::tk::run(async move {
        let rules = store::rules::all(&rt.pool).await?;
        let usage = agent::usage::summary(&rt).await?;
        let since = agent::usage::week_start(agent::usage::today(&rt)).format("%Y-%m-%d").to_string();
        let mut by = Vec::new();
        for row in store::usage::by_bot(&rt.pool, &since).await? {
          let name = store::bots::get(&rt.pool, &row.bot_id).await.map(|b| b.name).unwrap_or_else(|_| "—".into());
          by.push((name, row.prompt_tokens + row.completion_tokens));
        }
        let disk = agent::api::computer::disk(&rt);
        let backups = agent::api::computer::snapshots(&rt).len();
        let links = store::slack::links(&rt.pool).await?;
        let activity = activity::load(&rt).await.ok();
        let update_at = agent::api::computer::scheduled_update(&rt).await;
        Ok((rules, usage, by, disk, backups, links, activity, update_at))
      })
      .await;
      let _ = this.update(cx, |this, cx| {
        if let Ok((rules, usage, by, disk, backups, links, activity, update_at)) = r {
          this.update_at = update_at;
          this.links = links;
          this.activity = activity;
          this.rules = rules;
          this.usage = usage;
          this.by_bot = by;
          this.disk = Some(disk);
          this.backups = backups;
        }
        cx.notify();
      });
    })
    .detach();
  }

  pub fn save_key(&mut self, key: String, cx: &mut Context<Self>) {
    let key = key.trim().to_string();
    if key.is_empty() {
      return;
    }
    self.status = Some(t("Checking…").into());
    let rt = self.rt.clone();
    cx.spawn(async move |this, cx| {
      let r = crate::tk::run(async move {
        grok::Client::new(key.clone(), None).models().await?;
        config::secret::set(config::secret::XAI_KEY, &key)?;
        let _ = rt.refresh_models().await;
        Ok(())
      })
      .await;
      let _ = this.update(cx, |this, cx| {
        match r {
          Ok(()) => {
            this.has_key = true;
            this.status = Some(t("Connected. Your key is stored in the macOS Keychain.").into());
            this.key.update(cx, |k, cx| k.set_text("", cx));
          }
          Err(e) => this.status = Some(format!("{}: {e}", t("That key didn't work"))),
        }
        cx.notify();
      });
    })
    .detach();
  }

}

pub fn capital(s: &str) -> String {
  let mut c = s.chars();
  match c.next() {
    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    None => String::new(),
  }
}

/// A labeled settings row.
pub fn row(label: impl Into<SharedString>, desc: Option<&str>, control: impl IntoElement, cx: &App) -> AnyElement {
  let ink = ink(cx);
  div()
    .flex()
    .items_center()
    .gap(px(16.0))
    .py(px(10.0))
    .border_b_1()
    .border_color(ink.border)
    .child(
      div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(div().text_size(px(13.5)).child(label.into()))
        .when_some(desc, |d, s| d.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(SharedString::from(s.to_string())))),
    )
    .child(div().flex_none().child(control))
    .into_any_element()
}

pub fn heading(title: impl Into<SharedString>, cx: &App) -> AnyElement {
  let ink = ink(cx);
  div().pt(px(18.0)).pb(px(4.0)).text_size(px(12.0)).font_weight(gpui::FontWeight::SEMIBOLD).text_color(ink.dimmed).child(title.into().to_uppercase()).into_any_element()
}

pub fn switch(id: &'static str, on: bool, rt: &Runtime, set: fn(&mut config::Settings, bool)) -> guise::Switch {
  let rt = rt.clone();
  guise::Switch::new(id).checked(on).color(guise::ColorName::Violet).on_change(move |_, _, cx| {
    save(&rt, |s| set(s, !on));
    cx.refresh_windows();
  })
}

impl Render for Dialog {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let mut nav = div().w(px(200.0)).flex_none().flex().flex_col().gap(px(2.0)).p(px(10.0)).border_r_1().border_color(ink.border);
    for (id, label, icon) in PAGES {
      let active = self.page == id;
      nav = nav.child(
        div()
          .id(id)
          .flex()
          .items_center()
          .gap(px(8.0))
          .px(px(10.0))
          .py(px(7.0))
          .rounded(px(8.0))
          .cursor_pointer()
          .when(active, |d| d.bg(ink.hover))
          .hover(|s| s.bg(ink.hover))
          .on_click(cx.listener(move |this, _, _, cx| {
            this.page = id;
            this.refresh(cx);
          }))
          .child(div().text_color(ink.dimmed).child(guise::Icon::new(icon).size(Size::Sm)))
          .child(t(label)),
      );
    }
    let content = match self.page {
      "computer" => computer::render(self, window, cx),
      "providers" => providers::render(self, window, cx),
      "usage" => usage::render(self, cx),
      "team" => general::team(self, cx),
      "updates" => updates::render(self, cx),
      "rules" => rules::render(self, cx),
      "activity" => activity::render(self, cx),
      _ => general::render(self, window, cx),
    };
    let root = self.root.clone();
    div()
      .absolute()
      .top_0()
      .left_0()
      .size_full()
      .flex()
      .items_center()
      .justify_center()
      .bg(gpui::black().opacity(0.45))
      .child(
        div()
          .id("settings")
          .w(px(900.0))
          .h(px(640.0))
          .flex()
          .flex_col()
          .rounded(px(14.0))
          .overflow_hidden()
          .bg(ink.body)
          .border_1()
          .border_color(ink.border)
          .shadow_lg()
          .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
          .child(
            div()
              .h(px(48.0))
              .flex()
              .items_center()
              .justify_between()
              .px(px(16.0))
              .border_b_1()
              .border_color(ink.border)
              .child(div().flex().items_center().gap(px(8.0)).child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(t("Settings"))).children(managed(&self.rt, &ink)))
              .child(guise::CloseButton::new("close-settings").on_click(move |_, w, cx| {
                let _ = root.update(cx, |r, cx| r.close_modal(w, cx));
              })),
          )
          .child(div().flex().flex_1().min_h_0().child(nav).child(div().id("settings-body").flex_1().min_w_0().overflow_y_scroll().px(px(24.0)).pb(px(24.0)).child(div().w_full().child(content)))),
      )
  }
}

/// "Managed by <organization>" when an admin policy is installed, or why
/// the policy couldn't be read.
fn managed(rt: &agent::Runtime, ink: &crate::theme::Ink) -> Option<gpui::AnyElement> {
  if let Some(e) = rt.policy_error.read().ok().and_then(|e| e.clone()) {
    return Some(div().text_size(px(12.0)).text_color(ink.warning).child(format!("{} {e}", t("Couldn't read your admin's policy:"))).into_any_element());
  }
  let p = rt.policy();
  if !p.managed() {
    return None;
  }
  let who = if p.organization.is_empty() { t("your admin").to_string() } else { p.organization };
  Some(guise::Badge::new(crate::i18n::tf("Managed by {}", &[&who])).size(Size::Xs).variant(guise::Variant::Light).color(guise::ColorName::Violet).into_any_element())
}
