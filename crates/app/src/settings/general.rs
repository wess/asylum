use super::{heading, row, switch, Dialog};
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, Window};
use guise::{Button, Size, Variant};

pub fn render(d: &mut Dialog, _window: &mut Window, cx: &mut Context<Dialog>) -> AnyElement {
  let s = d.rt.settings();
  let rt = &d.rt;
  let policy = rt.policy();
  div()
    .flex()
    .flex_col()
    .child(heading(t("Account"), cx))
    .child(row(t("Your name"), Some(t("Bots use it when they write for you.")), div().w(px(240.0)).child(d.name.clone()), cx))
    .child(heading(t("Appearance"), cx))
    .child(row(t("Theme"), None, div().w(px(200.0)).child(d.theme.clone()), cx))
    .child(row(t("Language"), Some(t("New Bots write in this language unless you write in another.")), div().w(px(240.0)).child(d.language.clone()), cx))
    .child(heading(t("Bot"), cx))
    .child(row(t("Timezone"), Some(t("Routine schedules use this.")), div().w(px(240.0)).child(d.zone.clone()), cx))
    .child(row(
      t("Execution on Local Computer"),
      Some(if policy.local_exec.is_some_and(|max| policy.cap_local(s.local_exec) != s.local_exec || max == config::LocalExec::Never) {
        t("Limited by your admin: stricter settings win.")
      } else {
        t("Whether Bots may run commands and read files on this Mac, outside their computer.")
      }),
      div().w(px(200.0)).child(d.local.clone()),
      cx,
    ))
    .child(row(
      t("Auto-review"),
      Some(if policy.auto_review.is_some() { t("Required by your admin") } else { t("Checks each action before it runs and asks you first when needed.") }),
      switch("auto-review", policy.auto_review(s.auto_review), rt, |s, v| s.auto_review = v).disabled(policy.auto_review.is_some()),
      cx,
    ))
    .child(row(t("Auto-review Rules"), Some(t("Ask first and Allow automatically rules, in plain language.")), Button::new("open-rules", t("Edit rules")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, _, cx| {
      this.page = "rules";
      cx.notify();
    })), cx))
    .child(row(t("Memory"), Some(t("Bots keep stable preferences, important facts, and summaries of past work.")), switch("memory", s.memory, rt, |s, v| s.memory = v), cx))
    .child(row(t("Background work"), Some(t("Routines keep running while the window is closed.")), switch("background", s.background_work, rt, |s, v| s.background_work = v), cx))
    .child(row(t("Isolate the computer"), Some(t("Bots' shell commands can only write inside the workspace.")), switch("isolate", s.isolate_computer, rt, |s, v| s.isolate_computer = v), cx))
    .child(row(t("Show reasoning"), Some(t("Show the model's thinking, collapsed, above replies.")), switch("reasoning", s.show_reasoning, rt, |s, v| s.show_reasoning = v), cx))
    .child(row(
      t("Sync sections across your Macs"),
      Some(if s.sync_folder.is_empty() { t("Uses a folder in iCloud Drive.") } else { s.sync_folder.as_str() }),
      guise::Switch::new("sync").checked(!s.sync_folder.is_empty()).color(guise::ColorName::Violet).on_change({
        let rt = rt.clone();
        let on = s.sync_folder.is_empty();
        move |_, _, cx| {
          let folder = if on { agent::api::sync::icloud().display().to_string() } else { String::new() };
          super::save(&rt, |s| s.sync_folder = folder);
          cx.refresh_windows();
        }
      }),
      cx,
    ))
    .child(heading(t("Notifications"), cx))
    .child(row(t("Desktop notifications"), Some(t("When a Bot finishes or needs input. Quiet while Asylum is focused.")), switch("notifs", s.notifications, rt, |s, v| s.notifications = v), cx))
    .child(row(t("Sound"), None, switch("sound", s.notification_sound, rt, |s, v| s.notification_sound = v), cx))
    .child(heading(t("Composer"), cx))
    .child(row(t("Enter sends"), Some(t("Off: Enter adds a line and ⌘Enter sends.")), switch("enter", s.send_on_enter, rt, |s, v| s.send_on_enter = v), cx))
    .child(heading(t("Security Key"), cx))
    .child(row(t("Use hardware security keys"), Some(t("Let the computer's browser use your security key. Each use asks for approval.")), switch("keys", s.security_keys, rt, |s, v| s.security_keys = v), cx))
    .child(heading(t("Voice"), cx))
    .child(row(t("Voice features"), Some(t("Dictation, voice chat, and voice memos. Uses xAI's speech APIs and needs an xAI key.")), switch("voice-on", s.voice_enabled, rt, |s, v| s.voice_enabled = v), cx))
    .when(s.voice_enabled, |c| {
      c.child(row(t("Microphone"), None, div().w(px(240.0)).child(d.mic.clone()), cx))
        .child(row(t("Voice"), Some(t("The voice Bots speak with in voice chats and memos.")), div().w(px(200.0)).child(d.voice.clone()), cx))
    })
    .into_any_element()
}

/// Team Setup: the teams on this account and their members.
pub fn team(d: &mut Dialog, cx: &mut Context<Dialog>) -> AnyElement {
  let ink = ink(cx);
  div()
    .flex()
    .flex_col()
    .child(heading(t("Team Bots"), cx))
    .child(div().text_size(px(13.0)).text_color(ink.dimmed).child(t("Team Bots are Bots you set up once and share with your team through a link. Each teammate chats with it privately; it keeps team memory and private notes per person.")))
    .child(row(t("Clear connector preferences"), Some(t("Forget every “Always allow” you gave Team Bots for your personal connectors.")), Button::new("clear-prefs", t("Clear")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, _, cx| {
      let rt = this.rt.clone();
      cx.spawn(async move |this, cx| {
        let _ = crate::tk::run(async move {
          for r in store::rules::all(&rt.pool).await? {
            if r.text.contains("connector") && !r.locked {
              store::rules::delete(&rt.pool, &r.id).await?;
            }
          }
          Ok(())
        })
        .await;
        let _ = this.update(cx, |this, cx| {
          this.status = Some(t("Cleared.").into());
          cx.notify();
        });
      })
      .detach();
    })), cx))
    .child(heading(t("Linked Slack accounts"), cx))
    .child(div().text_size(px(12.5)).text_color(ink.dimmed).pb(px(6.0)).child(t("Team Bots in Slack answer linked teammates. Anyone else is asked, privately, to link. With no links, everyone in the workspace can talk to them.")))
    .children(d.links.clone().into_iter().map(|l| {
      let u = l.slack_user.clone();
      div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .py(px(4.0))
        .child(div().flex_1().text_size(px(13.0)).child(format!("{} · {}", if l.name.is_empty() { "—".to_string() } else { l.name.clone() }, l.slack_user)))
        .child(Button::new(gpui::SharedString::from(format!("unlink-{}", l.slack_user)), t("Remove")).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(move |this, _, _, cx| {
          let rt = this.rt.clone();
          let u = u.clone();
          cx.spawn(async move |this, cx| {
            let _ = crate::tk::run(async move { store::slack::unlink(&rt.pool, &u).await }).await;
            let _ = this.update(cx, |this, cx| this.refresh(cx));
          })
          .detach();
        })))
    }))
    .child(
      div()
        .flex()
        .gap(px(8.0))
        .child(div().w(px(220.0)).child(d.link_user.clone()))
        .child(div().w(px(160.0)).child(d.link_name.clone()))
        .child(Button::new("link", t("Link")).size(Size::Xs).on_click(cx.listener(|this, _, _, cx| {
          let (user, name) = (this.link_user.read(cx).text(), this.link_name.read(cx).text());
          if user.trim().is_empty() {
            return;
          }
          let rt = this.rt.clone();
          cx.spawn(async move |this, cx| {
            let _ = crate::tk::run(async move { store::slack::link(&rt.pool, &user, &name).await }).await;
            let _ = this.update(cx, |this, cx| {
              this.link_user.update(cx, |i, cx| i.set_text("", cx));
              this.link_name.update(cx, |i, cx| i.set_text("", cx));
              this.refresh(cx);
            });
          })
          .detach();
        }))),
    )
    .child(heading(t("Setup"), cx))
    .child(div().text_size(px(13.0)).text_color(ink.dimmed).child(t("Create a Team Bot from New → Create new Team Bot, or publish a personal Bot from its Share menu.")))
    .children(d.status.clone().map(|s| div().pt(px(8.0)).text_size(px(12.0)).text_color(ink.primary).child(s)))
    .into_any_element()
}
