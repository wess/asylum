//! Share: a template others can add as their own copy (public link or
//! team-only), and Team Bots — publish, the "How should your Team Bot
//! start?" choice, and the Team Bot's setup (plugins, secrets, skills, files).

use super::{section, Details};
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, PathPromptOptions, SharedString, Window};
use guise::{Badge, Button, Group, Size, Variant};

pub fn render(d: &mut Details, window: &mut Window, cx: &mut Context<Details>) -> AnyElement {
  let ink = ink(cx);
  let _ = window;
  let Some(b) = d.data.bot.clone() else { return div().into_any_element() };
  let mut col = div().flex().flex_col().gap(px(8.0));

  // Templates.
  col = col.child(section(t("Template"), cx)).child(div().text_size(px(12.5)).text_color(ink.dimmed).child(t(
    "A template gives someone their own copy of this Agent: identity, description, skills, and routines. Never the computer, sign-ins, memory, or history.",
  )));
  if let Some(tpl) = d.data.template.clone() {
    let visibility = if tpl.visibility == "team" { t("Team-only") } else { t("Public link") };
    let (bid1, bid2, vis) = (b.id.clone(), b.id.clone(), tpl.visibility.clone());
    let payload = tpl.payload.clone();
    col = col
      .child(div().flex().items_center().gap(px(6.0)).child(Badge::new(visibility).size(Size::Xs)).child(div().text_size(px(12.0)).text_color(ink.dimmed).child(crate::i18n::tf("Updated {}", &[&chrono::DateTime::from_timestamp_millis(tpl.updated).map(|t| t.format("%b %-d").to_string()).unwrap_or_default()]))))
      .child(
        Group::new()
          .gap(Size::Xs)
          .wrap(true)
          .child(Button::new("copy-link", t("Copy link")).size(Size::Xs).on_click(cx.listener(move |this, _, _, cx| {
            if let Ok(t2) = serde_json::from_str::<agent::template::Template>(&payload) {
              if let Ok(link) = agent::template::link(&t2) {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(link));
                this.status = Some(t("Link copied").into());
                cx.notify();
              }
            }
          })))
          .child(Button::new("update-tpl", t("Update template")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| create(this, &bid1, &vis, cx))))
          .child(Button::new("vis-tpl", if tpl.visibility == "team" { t("Make public link") } else { t("Make team-only") }).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(move |this, _, _, cx| {
            let other = if this.data.template.as_ref().is_some_and(|t| t.visibility == "team") { "public" } else { "team" };
            create(this, &bid2, other, cx);
          }))),
      );
    if let Ok(t2) = serde_json::from_str::<agent::template::Template>(&tpl.payload) {
      col = col.child(div().text_size(px(12.0)).text_color(ink.dimmed).child(crate::i18n::tf("View template details: {} skills, {} routines", &[&t2.skills.len().to_string(), &t2.routines.len().to_string()])));
      for w in agent::template::warnings(&t2) {
        col = col.child(div().text_size(px(12.0)).text_color(ink.warning).child(SharedString::from(w)));
      }
    }
  } else {
    let (b1, b2) = (b.id.clone(), b.id.clone());
    col = col
      .child(div().text_size(px(12.0)).text_color(ink.warning).child(t("Strip API keys, internal URLs, and customer data before sharing.")))
      .child(
        Group::new()
          .gap(Size::Xs)
          .child(Button::new("create-public", t("Create template")).size(Size::Xs).on_click(cx.listener(move |this, _, _, cx| create(this, &b1, "public", cx))))
          .child(Button::new("create-team", t("Team-only template")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| create(this, &b2, "team", cx)))),
      );
  }

  // Team Bots.
  col = col.child(section(t("Team"), cx));
  if b.is_team() {
    let ready = agent::api::team::ready(&b);
    let (b1, b2) = (b.id.clone(), b.id.clone());
    col = col
      .child(div().flex().gap(px(6.0)).child(Badge::new(if b.published { t("Published") } else { t("Not published") }).size(Size::Xs).color(if b.published { guise::ColorName::Green } else { guise::ColorName::Gray })))
      .when_some(ready.err(), |c, e| c.child(div().text_size(px(12.0)).text_color(ink.warning).child(SharedString::from(e.to_string()))))
      .child(
        Group::new()
          .gap(Size::Xs)
          .wrap(true)
          .child(Button::new("publish", if b.published { t("Unpublish") } else { t("Publish to team") }).size(Size::Xs).on_click(cx.listener(move |this, _, _, cx| {
            let rt = this.rt.clone();
            let on = !this.data.bot.as_ref().is_some_and(|b| b.published);
            let id = b1.clone();
            this.run(cx, async move { agent::api::team::publish(&rt, &id, on).await }, move |this, link, cx| {
              if on {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(link));
                this.status = Some(t("Published. Team link copied.").into());
              }
              this.load(cx);
            });
          })))
          .when(b.published, |g| {
            g.child(Button::new("team-link", t("Copy link")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| {
              let rt = this.rt.clone();
              let id = b2.clone();
              this.run(cx, async move { agent::api::team::link(&rt, &id).await }, |this, link, cx| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(link));
                this.status = Some(t("Link copied").into());
                cx.notify();
              });
            })))
          }),
      );
    // Setup rows.
    col = col.child(section(t("Setup"), cx));
    let files = agent::api::team::files(&d.rt, &b);
    let rows: [(&str, String); 4] = [
      ("Plugins", t("Connected apps are shared with every Agent").to_string()),
      ("Secrets", format!("{}", d.data.secrets.len())),
      ("Skills", format!("{}", d.data.enabled.len())),
      ("Files", format!("{}", files.len())),
    ];
    for (label, count) in rows {
      col = col.child(
        div()
          .flex()
          .items_center()
          .gap(px(8.0))
          .p(px(8.0))
          .rounded(px(8.0))
          .bg(ink.surface)
          .child(div().flex_1().child(t(label)))
          .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(count))
          .child(Button::new(SharedString::from(format!("add-{label}")), t("Add")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, w, cx| match label {
            "Plugins" => w.dispatch_action(Box::new(crate::actions::Marketplace), cx),
            "Secrets" => {
              this.tab = super::Tab::Secrets;
              this.adding_secret = true;
              cx.notify();
            }
            "Skills" => {
              this.tab = super::Tab::Skills;
              cx.notify();
            }
            _ => add_file(this, cx),
          }))),
      );
    }
    for f in files {
      col = col.child(div().pl(px(12.0)).text_size(px(12.0)).text_color(ink.dimmed).child(f.name));
    }
    col = col.child(section(t("Bring to your team's Slack"), cx)).child(slack(d, &b, cx));
  } else {
    let (b1, b2) = (b.id.clone(), b.id.clone());
    col = col
      .child(div().text_size(px(12.5)).text_color(ink.dimmed).child(t("Publish to Team makes a Team Agent: one Agent the whole team chats with privately, with team memory and shared setup.")))
      .child(div().text_size(px(13.0)).font_weight(gpui::FontWeight::MEDIUM).child(t("How should your Team Agent start?")))
      .child(
        Group::new()
          .gap(Size::Xs)
          .wrap(true)
          .child(Button::new("copy-bot", crate::i18n::tf("Copy {}", &[&b.name])).size(Size::Xs).on_click(cx.listener(move |this, _, _, cx| {
            let rt = this.rt.clone();
            let id = b1.clone();
            let memories: Vec<String> = this.data.memory.iter().filter(|m| picked(this, m)).map(|m| m.id.clone()).collect();
            let move_routines = this.team_routines;
            let root = this.root.clone();
            this.run(cx, async move { agent::api::team::convert(&rt, &id, agent::api::team::Start::Copy { memories, move_routines }).await }, move |_, (_, c), cx| {
              let _ = root.update(cx, |r, cx| {
                r.reload(cx);
                r.pending_open(c.id, cx);
              });
            });
          })))
          .child(Button::new("fresh", t("Start fresh")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| {
            let rt = this.rt.clone();
            let id = b2.clone();
            let root = this.root.clone();
            this.run(cx, async move { agent::api::team::convert(&rt, &id, agent::api::team::Start::Fresh).await }, move |_, (_, c), cx| {
              let _ = root.update(cx, |r, cx| {
                r.reload(cx);
                r.pending_open(c.id, cx);
              });
            });
          }))),
      )
      .child(div().text_size(px(11.5)).text_color(ink.dimmed).child(t("Copying brings facts and summaries as team memory; personal preferences stay private.")))
      .child(picker(d, cx));
  }
  col.into_any_element()
}

fn create(d: &mut Details, bot: &str, visibility: &str, cx: &mut Context<Details>) {
  let rt = d.rt.clone();
  let (bot, vis) = (bot.to_string(), visibility.to_string());
  d.run(
    cx,
    async move { agent::template::share(&rt, &bot, &vis).await },
    |this, link, cx| {
      cx.write_to_clipboard(gpui::ClipboardItem::new_string(link));
      this.status = Some(t("Template ready. Link copied.").into());
      this.load(cx);
    },
  );
}

fn add_file(d: &mut Details, cx: &mut Context<Details>) {
  let Some(bot) = d.data.bot.clone() else { return };
  let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: Some(t("Upload file").into()) });
  let rt = d.rt.clone();
  cx.spawn(async move |this, cx| {
    if let Ok(Ok(Some(paths))) = rx.await {
      let mut errors = Vec::new();
      for p in paths {
        if let Err(e) = agent::api::team::add_file(&rt, &bot, &p) {
          errors.push(e.to_string());
        }
      }
      let _ = this.update(cx, |this, cx| {
        this.status = errors.into_iter().next().or(Some(t("Uploaded.").into()));
        cx.notify();
      });
    }
  })
  .detach();
}

/// Bring a Team Bot to Slack: create its app from a manifest, paste the
/// two tokens, and it answers DMs and @mentions over Socket Mode.
fn slack(d: &mut Details, b: &store::Bot, cx: &mut Context<Details>) -> AnyElement {
  let ink = ink(cx);
  let awaiting = d.data.slack.as_ref().is_some_and(|a| a.status == agent::api::slack::AWAITING);
  if let Some(app) = d.data.slack.clone().filter(|_| !awaiting) {
    let id = b.id.clone();
    return div()
      .flex()
      .flex_col()
      .gap(px(6.0))
      .child(div().flex().items_center().gap(px(6.0)).child(Badge::new(t("Connected")).size(Size::Xs).color(guise::ColorName::Green)).child(div().text_size(px(12.5)).child(SharedString::from(app.team.clone()))))
      .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("Answers every DM; in channels it answers when mentioned and then follows the thread. Invite it with /invite.")))
      .child(Button::new("slack-remove", t("Remove from Slack")).size(Size::Xs).variant(Variant::Light).color(guise::ColorName::Red).on_click(cx.listener(move |this, _, _, cx| {
        let rt = this.rt.clone();
        let id = id.clone();
        this.run(cx, async move { agent::api::slack::remove(&rt, &id).await }, |this, _, cx| this.load(cx));
      })))
      .into_any_element();
  }
  if !b.published {
    return div().text_size(px(12.5)).text_color(ink.dimmed).child(t("Connect after publishing.")).into_any_element();
  }
  let (i1, i2, i3) = (b.id.clone(), b.id.clone(), b.id.clone());
  let approval: AnyElement = if awaiting {
    div()
      .flex()
      .flex_col()
      .gap(px(6.0))
      .child(div().flex().items_center().gap(px(6.0)).child(Badge::new(t("Awaiting approval")).size(Size::Xs).color(guise::ColorName::Yellow)).child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("Your Slack admin needs to approve the app. Once they do, install it and connect below."))))
      .child(
        Group::new()
          .gap(Size::Xs)
          .child(Button::new("slack-check", t("Check approval")).size(Size::Xs).variant(Variant::Light).on_click(|_, _, cx| cx.open_url(agent::api::slack::APPS_URL)))
          .child(Button::new("slack-cancel", t("Cancel request")).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(move |this, _, _, cx| {
            let rt = this.rt.clone();
            let id = i3.clone();
            this.run(cx, async move { agent::api::slack::remove(&rt, &id).await }, |this, _, cx| this.load(cx));
          }))),
      )
      .into_any_element()
  } else {
    Button::new("slack-request", t("My workspace needs admin approval")).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(move |this, _, _, cx| {
      let rt = this.rt.clone();
      let id = i3.clone();
      this.run(cx, async move { agent::api::slack::request_approval(&rt, &id).await }, |this, _, cx| this.load(cx));
    })).into_any_element()
  };
  div()
    .flex()
    .flex_col()
    .gap(px(6.0))
    .child(div().text_size(px(12.5)).child(t("1. Create the Slack app (it opens Slack with everything filled in), then install it to your workspace.")))
    .child(Button::new("slack-create", t("Create Slack app")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| {
      let rt = this.rt.clone();
      let id = i1.clone();
      this.run(cx, async move { agent::api::slack::manifest_url(&rt, &id).await }, |_, url, cx| cx.open_url(&url));
    })))
    .child(div().text_size(px(12.5)).child(t("2. Paste the Bot User OAuth Token and an app-level token with connections:write.")))
    .child(d.slack_bot.clone())
    .child(d.slack_app.clone())
    .child(Button::new("slack-connect", t("Connect")).size(Size::Xs).on_click(cx.listener(move |this, _, _, cx| {
      let (bt, at) = (this.slack_bot.read(cx).text(), this.slack_app.read(cx).text());
      let rt = this.rt.clone();
      let id = i2.clone();
      this.status = Some(t("Connecting…").into());
      this.run(cx, async move { agent::api::slack::connect(&rt, &id, &bt, &at, None).await }, |this, _, cx| {
        this.status = Some(t("Connected.").into());
        this.slack_bot.update(cx, |i, cx| i.set_text("", cx));
        this.slack_app.update(cx, |i, cx| i.set_text("", cx));
        this.load(cx);
      });
    })))
    .child(approval)
    .into_any_element()
}

/// Whether a memory goes to the Team Bot: facts and summaries by default,
/// preferences not, each flippable.
fn picked(d: &Details, m: &store::Memory) -> bool {
  (m.kind != "preference") != d.team_flip.contains(&m.id)
}

/// Choose which memories become team memory, and whether routines move.
fn picker(d: &mut Details, cx: &mut Context<Details>) -> AnyElement {
  let ink = ink(cx);
  let mut col = div().flex().flex_col().gap(px(2.0)).pt(px(6.0));
  if !d.data.memory.is_empty() {
    col = col.child(div().text_size(px(12.0)).font_weight(gpui::FontWeight::MEDIUM).pb(px(2.0)).child(t("Memories to bring")));
  }
  for m in d.data.memory.clone() {
    let on = picked(d, &m);
    let id = m.id.clone();
    col = col.child(
      div()
        .id(SharedString::from(format!("pick-{}", m.id)))
        .flex()
        .items_center()
        .gap(px(8.0))
        .py(px(2.0))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
          if !this.team_flip.remove(&id) {
            this.team_flip.insert(id.clone());
          }
          cx.notify();
        }))
        .child(guise::Checkbox::new(SharedString::from(format!("pick-ck-{}", m.id))).checked(on))
        .child(div().flex_1().min_w_0().truncate().text_size(px(12.5)).child(SharedString::from(m.content.clone())))
        .child(div().text_size(px(11.0)).text_color(ink.dimmed).child(SharedString::from(m.kind.clone()))),
    );
  }
  let routines = d.team_routines;
  col
    .child(
      div()
        .id("pick-routines")
        .flex()
        .items_center()
        .gap(px(8.0))
        .pt(px(6.0))
        .cursor_pointer()
        .on_click(cx.listener(|this, _, _, cx| {
          this.team_routines = !this.team_routines;
          cx.notify();
        }))
        .child(guise::Checkbox::new("pick-routines-ck").checked(routines))
        .child(div().text_size(px(12.5)).child(t("Move this Agent's routines to the Team Agent"))),
    )
    .into_any_element()
}
