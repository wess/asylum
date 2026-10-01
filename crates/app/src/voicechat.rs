//! Live voice chat with a Bot: one call at a time, 1:1 only. The call bar
//! shows the state, a level meter, Mute, the live transcript, voice
//! settings, and Hang up — and "Return to the voice chat" from elsewhere.
//! Afterwards the chat gets a Voice chat card and the Bot does any
//! follow-up; you're asked how the call went.

use crate::chat::ChatPane;
use crate::i18n::t;
use crate::root::Root;
use crate::theme::ink;
use crate::tk;
use gpui::prelude::*;
use gpui::{div, px, Context, SharedString, WeakEntity, Window};
use guise::{ActionIcon, Button, IconName, Size, Variant};
use voice::realtime::{Config, Handle, Update};

#[derive(Clone, Debug, PartialEq)]
pub enum State {
  Connecting,
  Live,
  Feedback,
  Failed(String),
}

pub struct Call {
  pub chat: String,
  pub bot: store::Bot,
  pub root: WeakEntity<Root>,
  pub state: State,
  pub handle: Option<Handle>,
  pub muted: bool,
  pub level: f32,
  pub speaking: bool,
  pub lines: Vec<(bool, String)>,
  pub partial_user: String,
  pub partial_bot: String,
  pub tasks: Vec<String>,
  pub started: std::time::Instant,
  pub show_transcript: bool,
  pub show_settings: bool,
}

pub fn start(pane: &mut ChatPane, window: &mut Window, cx: &mut Context<ChatPane>) {
  if !pane.rt.settings().voice_enabled {
    return;
  }
  if pane.is_group() {
    pane.toast(t("Voice chat isn't available in group chats."), cx);
    return;
  }
  let Some(bot) = pane.bot().cloned() else { return };
  if bot.is_team() && !bot.owner.is_empty() && bot.owner != pane.rt.settings().user_name {
    pane.toast(t("Voice chat isn't available with teammates' Team Agents."), cx);
    return;
  }
  if config::secret::xai_key().is_none() {
    pane.toast(t("Voice chat uses xAI's realtime voice. Add your xAI API key in Settings → Providers."), cx);
    return;
  }
  let chat = pane.id.clone();
  let _ = pane.root.update(cx, |root, cx| {
    if let Some(existing) = root.voice.clone() {
      if existing.read(cx).chat == chat {
        return;
      }
      let (bot, chat) = (bot.clone(), chat.clone());
      crate::root::dialogs::confirm(root, t("End the current voice chat?"), t("Only one voice chat can run at a time."), t("End and Chat"), window, cx, move |r, w, cx| {
        if let Some(v) = r.voice.clone() {
          v.update(cx, |c, cx| c.hang_up(cx));
        }
        r.voice = None;
        begin(r, bot.clone(), chat.clone(), w, cx);
      });
      return;
    }
    begin(root, bot, chat, window, cx);
  });
}

fn begin(root: &mut Root, bot: store::Bot, chat: String, _window: &mut Window, cx: &mut Context<Root>) {
  let weak = cx.entity().downgrade();
  let rt = root.rt.clone();
  let call = cx.new(|cx| {
    let c = Call {
      chat: chat.clone(),
      bot: bot.clone(),
      root: weak,
      state: State::Connecting,
      handle: None,
      muted: false,
      level: 0.0,
      speaking: false,
      lines: Vec::new(),
      partial_user: String::new(),
      partial_bot: String::new(),
      tasks: Vec::new(),
      started: std::time::Instant::now(),
      show_transcript: false,
      show_settings: false,
    };
    let s = rt.settings();
    let (bid, cid) = (bot.id.clone(), chat.clone());
    cx.spawn(async move |this, cx| {
      let r = tk::run(async move {
        let key = config::secret::xai_key().ok_or_else(|| anyhow::anyhow!("no xAI key"))?;
        let instructions = agent::api::voice::instructions(&rt, &bid, &cid).await?;
        let cfg = Config { instructions, voice: s.voice.clone(), speed: s.voice_speed, language: s.voice_language.clone(), device: s.microphone.clone(), tools: agent::api::voice::tools() };
        let session = voice::realtime::connect(&key, "", cfg).await?;
        Ok(session.split())
      })
      .await;
      match r {
        Ok((handle, mut rx)) => {
          let _ = this.update(cx, |c: &mut Call, cx| {
            c.handle = Some(handle);
            cx.notify();
          });
          while let Some(u) = rx.recv().await {
            if this.update(cx, |c: &mut Call, cx| c.on_update(u, cx)).is_err() {
              break;
            }
          }
        }
        Err(e) => {
          let msg = e.to_string();
          let text = if msg.to_lowercase().contains("microphone") { msg } else { t("The realtime connection failed.").to_string() };
          let _ = this.update(cx, |c: &mut Call, cx| {
            c.state = State::Failed(text);
            cx.notify();
          });
        }
      }
    })
    .detach();
    c
  });
  root.voice = Some(call);
  cx.notify();
}

impl Call {
  fn on_update(&mut self, u: Update, cx: &mut Context<Self>) {
    match u {
      Update::Connected => {
        self.state = State::Live;
        self.started = std::time::Instant::now();
      }
      Update::User(t) => self.partial_user = t,
      Update::UserDone(t) => {
        self.partial_user.clear();
        if !t.trim().is_empty() {
          self.lines.push((true, t));
        }
      }
      Update::Assistant(d) => self.partial_bot.push_str(&d),
      Update::AssistantDone => {
        let text = std::mem::take(&mut self.partial_bot);
        if !text.trim().is_empty() {
          self.lines.push((false, text));
        }
        self.speaking = false;
      }
      Update::Speaking(s) => self.speaking = s,
      Update::Level(l) => self.level = l,
      Update::Tool { call, name, args } => {
        if name == "note_task" {
          let v: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
          self.tasks.push(v["task"].as_str().unwrap_or_default().to_string());
        }
        if let Some(h) = &self.handle {
          h.tool_result(&call, "{\"ok\":true,\"note\":\"Queued for right after the call.\"}");
        }
      }
      Update::Error(e) => {
        if self.state == State::Connecting {
          self.state = State::Failed(e);
        } else {
          self.lines.push((false, format!("[{}]", t("The call ended unexpectedly."))));
          self.end(cx);
        }
      }
      Update::Closed => {
        if self.state == State::Live {
          self.end(cx);
        }
      }
    }
    cx.notify();
  }

  pub fn transcript(&self) -> String {
    let who = |u: bool| if u { t("You") } else { self.bot.name.as_str() };
    let mut s: Vec<String> = self.lines.iter().map(|(u, l)| format!("**{}:** {l}", who(*u))).collect();
    if !self.tasks.is_empty() {
      s.push(format!("\n{}\n{}", t("Follow-up:"), self.tasks.iter().map(|x| format!("- {x}")).collect::<Vec<_>>().join("\n")));
    }
    s.join("\n\n")
  }

  pub fn hang_up(&mut self, cx: &mut Context<Self>) {
    if let Some(h) = self.handle.take() {
      h.hangup();
    }
    self.end(cx);
  }

  fn end(&mut self, cx: &mut Context<Self>) {
    if matches!(self.state, State::Feedback) {
      return;
    }
    let secs = self.started.elapsed().as_secs();
    let transcript = self.transcript();
    let was_live = self.state == State::Live;
    self.state = State::Feedback;
    self.handle = None;
    if was_live {
      if let Some(rt) = self.root.upgrade().map(|r| r.read(cx).rt.clone()) {
        let (bot, chat) = (self.bot.id.clone(), self.chat.clone());
        tk::spawn(async move { agent::api::voice::finish(&rt, &bot, &chat, secs, &transcript).await });
      }
    }
    cx.notify();
  }

  fn close(&mut self, cx: &mut Context<Self>) {
    let _ = self.root.update(cx, |r, cx| {
      r.voice = None;
      cx.notify();
    });
  }
}

impl Render for Call {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let elsewhere = self.root.upgrade().is_some_and(|r| r.read(cx).active.as_deref() != Some(self.chat.as_str()));
    let secs = self.started.elapsed().as_secs();
    let status: SharedString = match &self.state {
      State::Connecting => t("Connecting…").into(),
      State::Live if self.speaking => format!("{} · {}:{:02}", t("Speaking"), secs / 60, secs % 60).into(),
      State::Live => format!("{}:{:02}", secs / 60, secs % 60).into(),
      State::Feedback => t("How was the call?").into(),
      State::Failed(e) => format!("{} {e}", t("Voice chat failed.")).into(),
    };
    let mut bar = div()
      .flex()
      .items_center()
      .gap(px(8.0))
      .px(px(12.0))
      .py(px(8.0))
      .rounded_full()
      .bg(ink.surface)
      .border_1()
      .border_color(if self.state == State::Live { ink.primary } else { ink.border })
      .shadow_lg()
      .child(crate::avatar::face(&self.bot, 26.0, cx))
      .child(div().flex().flex_col().child(div().text_size(px(13.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(self.bot.name.clone())).child(div().text_size(px(11.5)).text_color(ink.dimmed).child(status)));
    match &self.state {
      State::Live | State::Connecting => {
        bar = bar
          .child(div().w(px(60.0)).h(px(4.0)).rounded_full().bg(ink.hover).child(div().h_full().rounded_full().bg(ink.primary).w(px((self.level * 400.0).min(60.0)))))
          .child(ActionIcon::new("mute", if self.muted { IconName::MicOff } else { IconName::Mic }).variant(if self.muted { Variant::Filled } else { Variant::Subtle }).on_click(cx.listener(|c, _, _, cx| {
            c.muted = !c.muted;
            if let Some(h) = &c.handle {
              h.mute(c.muted);
            }
            cx.notify();
          })))
          .child(ActionIcon::new("transcript", IconName::ScrollText).variant(Variant::Subtle).on_click(cx.listener(|c, _, _, cx| {
            c.show_transcript = !c.show_transcript;
            cx.notify();
          })))
          .child(ActionIcon::new("vsettings", IconName::Settings2).variant(Variant::Subtle).on_click(cx.listener(|c, _, _, cx| {
            c.show_settings = !c.show_settings;
            cx.notify();
          })));
        if elsewhere {
          let chat = self.chat.clone();
          bar = bar.child(Button::new("return", t("Return to the voice chat")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |c, _, w, cx| {
            let chat = chat.clone();
            let _ = c.root.update(cx, |r, cx| r.open(&chat, w, cx));
          })));
        }
        bar = bar.child(ActionIcon::new("hangup", IconName::PhoneOff).variant(Variant::Filled).color(guise::ColorName::Red).on_click(cx.listener(|c, _, _, cx| c.hang_up(cx))));
      }
      State::Feedback => {
        bar = bar
          .child(Button::new("good", t("Good")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|c, _, _, cx| c.close(cx))))
          .child(Button::new("bad", t("Bad")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(|c, _, _, cx| c.close(cx))));
      }
      State::Failed(_) => {
        bar = bar.child(Button::new("dismiss", t("Dismiss")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(|c, _, _, cx| c.close(cx))));
      }
    }
    let mut col = div().absolute().top(px(52.0)).left_0().right_0().flex().flex_col().items_center().gap(px(6.0)).child(bar);
    if self.show_transcript {
      let mut tr = div().id("live-transcript").w(px(460.0)).max_h(px(260.0)).overflow_y_scroll().flex().flex_col().gap(px(6.0)).p(px(12.0)).rounded(px(12.0)).bg(ink.body).border_1().border_color(ink.border).shadow_lg();
      for (u, l) in &self.lines {
        tr = tr.child(div().text_size(px(12.5)).child(format!("{}: {l}", if *u { t("You") } else { self.bot.name.as_str() })));
      }
      if !self.partial_user.is_empty() {
        tr = tr.child(div().text_size(px(12.5)).text_color(ink.dimmed).child(format!("{}: {}", t("You"), self.partial_user)));
      }
      if !self.partial_bot.is_empty() {
        tr = tr.child(div().text_size(px(12.5)).text_color(ink.dimmed).child(format!("{}: {}", self.bot.name, self.partial_bot)));
      }
      if self.lines.is_empty() && self.partial_user.is_empty() && self.partial_bot.is_empty() {
        tr = tr.child(div().text_size(px(12.5)).text_color(ink.dimmed).child(t("The transcript appears as you talk.")));
      }
      col = col.child(tr);
    }
    if self.show_settings {
      col = col.child(self.settings_panel(cx));
    }
    col
  }
}

pub const SPEEDS: [f32; 5] = [0.7, 0.85, 1.0, 1.2, 1.5];
pub const LANGUAGES: [(&str, &str); 10] = [("auto", "Auto-detect"), ("en", "English"), ("es", "Español"), ("fr", "Français"), ("de", "Deutsch"), ("pt", "Português"), ("it", "Italiano"), ("ja", "日本語"), ("ko", "한국어"), ("hi", "हिन्दी")];

impl Call {
  /// Voice, Speed, and Language — applied to this call and saved for the next.
  fn settings_panel(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
    let ink = ink(cx);
    let Some(rt) = self.root.upgrade().map(|r| r.read(cx).rt.clone()) else { return div().into_any_element() };
    let s = rt.settings();
    let row = |label: &'static str| div().flex().flex_wrap().items_center().gap(px(4.0)).child(div().w(px(70.0)).text_color(ink.dimmed).child(t(label)));
    let mut voices = row("Voice");
    for v in crate::settings::VOICES {
      let on = s.voice.eq_ignore_ascii_case(v);
      let rt = rt.clone();
      voices = voices.child(crate::details::profile::chip(SharedString::from(format!("v-{v}")), crate::settings::capital(v).into(), on, &ink).on_click(cx.listener(move |c, _, _, cx| {
        crate::settings::save(&rt, |s| s.voice = v.to_string());
        c.reconfigure(&rt, cx);
      })));
    }
    let mut speeds = row("Speed");
    for sp in SPEEDS {
      let on = (s.voice_speed - sp).abs() < 0.01;
      let rt = rt.clone();
      speeds = speeds.child(crate::details::profile::chip(SharedString::from(format!("s-{sp}")), format!("{sp}×").into(), on, &ink).on_click(cx.listener(move |c, _, _, cx| {
        crate::settings::save(&rt, |s| s.voice_speed = sp);
        c.reconfigure(&rt, cx);
      })));
    }
    let mut langs = row("Language");
    for (code, name) in LANGUAGES {
      let on = s.voice_language == code;
      let rt = rt.clone();
      let label = if code == "auto" { t(name).to_string() } else { name.to_string() };
      langs = langs.child(crate::details::profile::chip(SharedString::from(format!("l-{code}")), label.into(), on, &ink).on_click(cx.listener(move |c, _, _, cx| {
        crate::settings::save(&rt, |s| s.voice_language = code.to_string());
        c.reconfigure(&rt, cx);
      })));
    }
    div()
      .w(px(460.0))
      .flex()
      .flex_col()
      .gap(px(8.0))
      .p(px(12.0))
      .rounded(px(12.0))
      .bg(ink.body)
      .border_1()
      .border_color(ink.border)
      .shadow_lg()
      .text_size(px(12.5))
      .child(voices)
      .child(speeds)
      .child(langs)
      .into_any_element()
  }

  fn reconfigure(&mut self, rt: &agent::Runtime, cx: &mut Context<Self>) {
    let s = rt.settings();
    if let Some(h) = &self.handle {
      h.configure(&Config { instructions: String::new(), voice: s.voice.clone(), speed: s.voice_speed, language: s.voice_language.clone(), device: String::new(), tools: agent::api::voice::tools() });
    }
    cx.notify();
  }
}
