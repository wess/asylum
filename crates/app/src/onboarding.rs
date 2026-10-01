//! First run: welcome, connect a model, a short tour, which tools you use
//! (it only shapes suggestions), and Meet a future teammate — pick a
//! suggested Bot or create your own.

use crate::i18n::t;
use crate::root::Root;
use crate::theme::ink;
use crate::tk;
use agent::Runtime;
use gpui::prelude::*;
use gpui::{div, px, Context, Entity, SharedString, WeakEntity, Window};
use guise::{Button, Group, IconName, Size, TextArea, TextInput, Variant};
use std::collections::BTreeSet;

pub const DONE_KEY: &str = "onboarded";

pub struct Suggestion {
  pub name: &'static str,
  pub label: &'static str,
  pub description: &'static str,
  pub skill: &'static str,
  pub tools: &'static [&'static str],
  pub avatar: &'static str,
}

pub const TEAMMATES: &[Suggestion] = &[
  Suggestion { name: "Chief of Staff", label: "Runs your day and routes work to the team", description: "Send a morning digest of what changed across my tools, flag decisions I owe, and hand work to the Agent whose job fits. Pull me in only for judgment calls.", skill: "Daily digest", tools: &["Gmail", "Slack", "Google Calendar", "Notion"], avatar: "bot:shape=round;eyes=happy;accessory=headset;tone=#7c5cff" },
  Suggestion { name: "Talent Scout", label: "Sources and screens candidates", description: "Find candidates who match the role, skip anyone already contacted, and draft outreach for my approval. Never send without approval.", skill: "Talent scout", tools: &["Gmail", "Notion"], avatar: "bot:shape=tall;eyes=ovals;accessory=antenna;tone=#1fb6ff" },
  Suggestion { name: "Inbox Triage", label: "Keeps your inbox at zero", description: "Sort new email into act, reply, read, and archive. Draft replies in my voice for approval.", skill: "Inbox triage", tools: &["Gmail"], avatar: "bot:shape=square;eyes=dots;accessory=none;tone=#13ce66" },
  Suggestion { name: "Bug Reproducer", label: "Turns bug reports into repro steps", description: "Reproduce reported bugs on the computer, capture evidence, and narrow to the smallest repro.", skill: "Bug reproduction", tools: &["GitHub", "Linear", "Jira", "Sentry"], avatar: "bot:shape=wide;eyes=visor;accessory=antenna;tone=#ff6b6b" },
  Suggestion { name: "Expense Manager", label: "Files expenses from receipts", description: "Collect receipts from email and the workspace, categorize spend, and prepare the report for my approval.", skill: "Expense report", tools: &["Gmail", "Google Drive"], avatar: "bot:shape=round;eyes=dots;accessory=bow;tone=#ffb020" },
  Suggestion { name: "Researcher", label: "Answers questions with sources", description: "Research questions thoroughly, cite every source, and save findings to the workspace.", skill: "Competitive research", tools: &["Notion", "Google Drive"], avatar: "bot:shape=tall;eyes=wink;accessory=halo;tone=#00c2a8" },
];

pub const TOOLS: [&str; 12] = ["Gmail", "Google Calendar", "Google Drive", "Slack", "GitHub", "Linear", "Jira", "Notion", "Sentry", "Salesforce", "Figma", "Stripe"];

#[derive(Default, Clone)]
pub struct Found {
  pub ollama: Vec<String>,
  pub claude: bool,
  pub codex: bool,
  pub litellm: bool,
}

pub struct Onboarding {
  rt: Runtime,
  root: WeakEntity<Root>,
  step: usize,
  found: Found,
  key: Entity<TextInput>,
  /// Which keyed provider is being connected (index into `KEYED`), and its address.
  keyed: usize,
  address: Entity<TextInput>,
  tools: BTreeSet<&'static str>,
  name: Entity<TextInput>,
  job: Entity<TextInput>,
  about: Entity<TextArea>,
  status: Option<String>,
}

/// Show onboarding on first run.
pub fn check(root: &mut Root, cx: &mut Context<Root>) {
  let rt = root.rt.clone();
  cx.spawn(async move |this, cx| {
    let done = tk::run(async move { Ok(store::state::get(&rt.pool, DONE_KEY).await?.is_some() || store::bots::count(&rt.pool).await? > 0) }).await.unwrap_or(true);
    if !done {
      let _ = this.update(cx, |r, cx| {
        r.onboard = true;
        cx.notify();
      });
    }
  })
  .detach();
}

pub fn open(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
  let rt = root.rt.clone();
  let weak = cx.entity().downgrade();
  let view = cx.new(|cx| {
    let key = cx.new(|cx| TextInput::new(cx).password(true).placeholder(t("Paste your API key")));
    let address = cx.new(|cx| TextInput::new(cx).placeholder("https://litellm.example.com/v1"));
    let name = cx.new(|cx| TextInput::new(cx).placeholder(t("Name")));
    let job = cx.new(|cx| TextInput::new(cx).placeholder(t("One primary job")));
    let about = cx.new(|cx| TextArea::new(cx).rows(3).placeholder(t("What it should do, and any rules")));
    let _ = window;
    let mut o = Onboarding { rt, root: weak, step: 0, found: Found::default(), key, keyed: 0, address, tools: BTreeSet::new(), name, job, about, status: None };
    o.detect(cx);
    o
  });
  root.set_modal(view, cx);
}

impl Onboarding {
  fn detect(&mut self, cx: &mut Context<Self>) {
    cx.spawn(async move |this, cx| {
      let found = tk::run(async move {
        let ollama = provider::discover(&provider::preset::build("ollama", "ollama")).await.unwrap_or_default();
        let litellm = provider::discover(&provider::preset::build("lite-llm", "litellm")).await.is_ok();
        Ok(Found { ollama, claude: provider::process::check_command("claude").is_ok(), codex: provider::process::check_command("codex").is_ok(), litellm })
      })
      .await
      .unwrap_or_default();
      let _ = this.update(cx, |o, cx| {
        o.found = found;
        cx.notify();
      });
    })
    .detach();
  }

  fn use_preset(&mut self, preset: &str, model: Option<String>, cx: &mut Context<Self>) {
    let mut p = provider::preset::build(preset, &preset.replace('-', ""));
    if preset == "ollama" {
      p.models = self.found.ollama.clone();
    }
    let name = p.name.clone();
    let model = model.or_else(|| p.models.first().cloned()).unwrap_or_default();
    crate::settings::save(&self.rt, |s| {
      if !s.providers.iter().any(|x| x.name == name) {
        if s.providers.is_empty() {
          s.providers.push(provider::preset::default_profile());
        }
        s.providers.push(p);
      }
      s.provider = name;
      s.model = model;
    });
    self.step = 2;
    cx.notify();
  }

  /// Connect the chosen provider with a pasted key (checked first, then
  /// kept in the Keychain) and make it the default.
  fn save_key(&mut self, cx: &mut Context<Self>) {
    let key = self.key.read(cx).text().trim().to_string();
    if key.is_empty() {
      self.status = Some(t("Paste the API key first.").into());
      cx.notify();
      return;
    }
    let (preset, _) = agent::api::providers::KEYED[self.keyed];
    let address = self.address.read(cx).text();
    let name = preset.replace('-', "");
    self.status = Some(t("Checking…").into());
    let rt = self.rt.clone();
    cx.spawn(async move |this, cx| {
      let r = tk::run(async move {
        let p = agent::api::providers::connect(&rt, preset, &name, &address, &key).await?;
        if preset == "xai" {
          let _ = rt.refresh_models().await;
        }
        anyhow::Ok(p)
      })
      .await;
      let _ = this.update(cx, |o, cx| match r {
        Ok(p) => {
          o.status = None;
          let (n, m) = (p.name.clone(), p.models.first().cloned().unwrap_or_default());
          crate::settings::save(&o.rt, |s| {
            s.provider = n;
            if !m.is_empty() {
              s.model = m;
            }
          });
          o.key.update(cx, |i, cx| i.set_text("", cx));
          o.step = 2;
          cx.notify();
        }
        Err(e) => {
          o.status = Some(e.to_string());
          cx.notify();
        }
      });
    })
    .detach();
  }

  fn finish(&mut self, pick: Option<&'static Suggestion>, window: &mut Window, cx: &mut Context<Self>) {
    let (name, label, about, avatar, skill) = match pick {
      Some(s) => (s.name.to_string(), s.label.to_string(), s.description.to_string(), s.avatar.to_string(), Some(s.skill)),
      None => (self.name.read(cx).text().trim().to_string(), self.job.read(cx).text().trim().to_string(), self.about.read(cx).text(), String::new(), None),
    };
    if name.is_empty() {
      self.status = Some(t("Give your teammate a name.").into());
      cx.notify();
      return;
    }
    let rt = self.rt.clone();
    let root = self.root.clone();
    let _ = root.update(cx, |r, cx| r.close_modal(window, cx));
    cx.spawn(async move |_, cx| {
      let r = tk::run(async move {
        let (b, c) = agent::api::bots::create(&rt, Some(&name)).await?;
        let mut p = b.profile();
        p.label = label;
        p.description = about;
        if !avatar.is_empty() {
          p.avatar = avatar;
        }
        agent::api::bots::update(&rt, &b.id, &p).await?;
        if let Some(sk) = skill {
          let s = agent::api::bots::add_packaged_skill(&rt, sk).await?;
          store::skills::enable(&rt.pool, &b.id, &s.id, true).await?;
        }
        store::state::set(&rt.pool, DONE_KEY, "1").await?;
        Ok(c)
      })
      .await;
      if let Ok(c) = r {
        let _ = root.update(cx, |r, cx| {
          r.onboard = false;
          r.reload(cx);
          r.pending_open(c.id, cx);
        });
      }
    })
    .detach();
  }
}

fn option(id: &'static str, icon: IconName, title: SharedString, sub: SharedString, ink: &crate::theme::Ink) -> gpui::Stateful<gpui::Div> {
  let hover = ink.hover;
  div()
    .id(id)
    .flex()
    .items_center()
    .gap(px(12.0))
    .p(px(12.0))
    .rounded(px(10.0))
    .border_1()
    .border_color(ink.border)
    .cursor_pointer()
    .hover(move |s| s.bg(hover))
    .child(div().text_color(ink.primary).child(guise::Icon::new(icon).size(Size::Md)))
    .child(div().flex().flex_col().child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(title)).child(div().text_size(px(12.0)).text_color(ink.dimmed).child(sub)))
}

impl Render for Onboarding {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let _ = window;
    let body: gpui::AnyElement = match self.step {
      0 => div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(14.0))
        .py(px(20.0))
        .child(gpui::img(std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/icon.png"))).size(px(88.0)))
        .child(div().text_size(px(24.0)).font_weight(gpui::FontWeight::BOLD).child(t("Meet your AI teammates")))
        .child(div().max_w(px(440.0)).text_center().text_color(ink.dimmed).child(t("Agents have names, jobs, and a computer of their own. They work in your real tools, remember what matters, and keep going while you're away.")))
        .child(Button::new("start", t("Get started")).size(Size::Lg).on_click(cx.listener(|o, _, _, cx| {
          o.step = 1;
          cx.notify();
        })))
        .into_any_element(),
      1 => {
        let f = self.found.clone();
        let mut col = div().flex().flex_col().gap(px(8.0)).child(div().text_size(px(18.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(t("Connect a model"))).child(div().text_size(px(13.0)).text_color(ink.dimmed).child(t("Agents think with the model you choose. You can add more providers later in Settings.")));
        if !f.ollama.is_empty() {
          let first = f.ollama.iter().find(|m| m.contains("qwen") || m.contains("llama3")).cloned().or_else(|| f.ollama.first().cloned());
          col = col.child(option("o-ollama", IconName::Cpu, t("Ollama on this Mac").into(), crate::i18n::tf("Found {} models — runs locally", &[&f.ollama.len().to_string()]).into(), &ink).on_click(cx.listener(move |o, _, _, cx| o.use_preset("ollama", first.clone(), cx))));
        }
        if f.litellm {
          col = col.child(option("o-litellm", IconName::Network, t("LiteLLM proxy").into(), "http://127.0.0.1:4000/v1".into(), &ink).on_click(cx.listener(|o, _, _, cx| o.use_preset("lite-llm", None, cx))));
        }
        if f.claude {
          col = col.child(option("o-claude", IconName::Terminal, t("Claude Code").into(), t("Uses your installed claude CLI").into(), &ink).on_click(cx.listener(|o, _, _, cx| o.use_preset("claude-code", None, cx))));
        }
        if f.codex {
          col = col.child(option("o-codex", IconName::Terminal, t("Codex").into(), t("Uses your installed codex CLI").into(), &ink).on_click(cx.listener(|o, _, _, cx| o.use_preset("codex", None, cx))));
        }
        // Any keyed provider: pick it, give the address if it has none of its own, paste the key.
        let mut kinds = div().flex().flex_wrap().gap(px(4.0));
        for (i, (_, label)) in agent::api::providers::KEYED.iter().enumerate() {
          let on = self.keyed == i;
          kinds = kinds.child(crate::details::profile::chip(SharedString::from(format!("keyed-{i}")), t(label).into(), on, &ink).on_click(cx.listener(move |o, _, _, cx| {
            o.keyed = i;
            o.status = None;
            cx.notify();
          })));
        }
        let preset = agent::api::providers::KEYED[self.keyed].0;
        col = col
          .child(div().pt(px(6.0)).text_size(px(12.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(t("Connect with an API key")))
          .child(kinds)
          .when(agent::api::providers::needs_endpoint(preset), |c| c.child(self.address.clone()))
          .child(div().flex().gap(px(8.0)).child(div().flex_1().child(self.key.clone())).child(Button::new("save-key", t("Connect")).on_click(cx.listener(|o, _, _, cx| o.save_key(cx)))))
          .child(div().text_size(px(11.5)).text_color(ink.dimmed).child(t("The key is checked, then kept in your Mac's Keychain. Nothing to set up in a terminal.")))
          .child(Button::new("skip-model", t("Set up later")).variant(Variant::Subtle).size(Size::Xs).on_click(cx.listener(|o, _, _, cx| {
            o.step = 2;
            cx.notify();
          })));
        col.into_any_element()
      }
      2 => {
        let item = |icon: IconName, title: &'static str, body: &'static str| {
          div().flex().gap(px(12.0)).p(px(12.0)).rounded(px(10.0)).bg(ink.surface).child(div().text_color(ink.primary).child(guise::Icon::new(icon).size(Size::Md))).child(div().flex().flex_col().gap(px(2.0)).child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(t(title))).child(div().text_size(px(13.0)).text_color(ink.dimmed).child(t(body))))
        };
        div()
          .flex()
          .flex_col()
          .gap(px(10.0))
          .child(div().text_size(px(18.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(t("How it works")))
          .child(item(IconName::Bot, "Agents", "Each Agent has a name, a job, its own conversation, and memory that builds over time. Message them like teammates."))
          .child(item(IconName::Monitor, "A shared computer", "Agents share one computer with a workspace, a browser that stays signed in, and a terminal. Watch any Agent's screen or take over."))
          .child(item(IconName::CalendarClock, "Routines", "Ask an Agent to do something every weekday at 8, or whenever an event arrives, and it will."))
          .child(Button::new("tour-next", t("Next")).on_click(cx.listener(|o, _, _, cx| {
            o.step = 3;
            cx.notify();
          })))
          .into_any_element()
      }
      3 => {
        let mut chips = div().flex().flex_wrap().gap(px(6.0));
        for tool in TOOLS {
          let on = self.tools.contains(tool);
          chips = chips.child(crate::details::profile::chip(SharedString::from(format!("tool-{tool}")), tool.into(), on, &ink).py(px(6.0)).px(px(12.0)).text_size(px(13.0)).on_click(cx.listener(move |o, _, _, cx| {
            if !o.tools.remove(tool) {
              o.tools.insert(tool);
            }
            cx.notify();
          })));
        }
        div()
          .flex()
          .flex_col()
          .gap(px(12.0))
          .child(div().text_size(px(18.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(t("Which tools do you use?")))
          .child(div().text_size(px(13.0)).text_color(ink.dimmed).child(t("This only shapes the teammates we suggest. Nothing gets connected.")))
          .child(chips)
          .child(Button::new("tools-next", t("Next")).on_click(cx.listener(|o, _, _, cx| {
            o.step = 4;
            cx.notify();
          })))
          .into_any_element()
      }
      _ => {
        let picked: Vec<&str> = self.tools.iter().copied().collect();
        let mut ranked: Vec<&'static Suggestion> = TEAMMATES.iter().collect();
        ranked.sort_by_key(|s| std::cmp::Reverse(s.tools.iter().filter(|t| picked.contains(t)).count()));
        let mut grid = div().flex().flex_wrap().gap(px(10.0));
        for (i, s) in ranked.into_iter().enumerate() {
          let bot = store::Bot { name: s.name.into(), avatar: s.avatar.into(), ..Default::default() };
          grid = grid.child(
            div()
              .id(SharedString::from(format!("mate-{i}")))
              .w(px(236.0))
              .flex()
              .flex_col()
              .gap(px(6.0))
              .p(px(12.0))
              .rounded(px(10.0))
              .border_1()
              .border_color(ink.border)
              .cursor_pointer()
              .hover(|st| st.bg(ink.hover))
              .on_click(cx.listener(move |o, _, w, cx| o.finish(Some(s), w, cx)))
              .child(div().flex().items_center().gap(px(8.0)).child(crate::avatar::face(&bot, 32.0, cx)).child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(s.name)))
              .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t(s.label))),
          );
        }
        div()
          .flex()
          .flex_col()
          .gap(px(12.0))
          .child(div().text_size(px(18.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(t("Meet a future teammate")))
          .child(grid)
          .child(div().pt(px(6.0)).text_size(px(13.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(t("Create your own")))
          .child(self.name.clone())
          .child(self.job.clone())
          .child(self.about.clone())
          .child(Group::new().child(Button::new("create-own", t("Create")).on_click(cx.listener(|o, _, w, cx| o.finish(None, w, cx)))))
          .into_any_element()
      }
    };
    let mut col = div().flex().flex_col().gap(px(10.0)).child(body);
    if let Some(s) = &self.status {
      col = col.child(div().text_size(px(12.0)).text_color(ink.primary).child(SharedString::from(s.clone())));
    }
    div().absolute().top_0().left_0().size_full().child(guise::Modal::new().width(560.0).child(div().id("onboarding").max_h(px(620.0)).overflow_y_scroll().child(col)))
  }
}
