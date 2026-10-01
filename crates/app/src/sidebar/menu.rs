//! Right-click menus for sidebar rows and section headers, plus inline
//! rename.

use crate::i18n::t;
use crate::root::Root;
use crate::state::Item;
use crate::menu::Menu;
use gpui::{AppContext, Context, Pixels, Point, Window};
use guise::{TextInput, TextInputEvent};

fn show(root: &mut Root, menu: Menu, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Root>) {
  root.menu = menu.show(at, 240.0, window, cx);
  cx.notify();
}

/// Build a handler that runs `f` against the root.
fn act(root: &Root, cx: &mut Context<Root>, f: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static) -> impl Fn(&mut Window, &mut gpui::App) + 'static {
  let _ = root;
  let me = cx.entity().downgrade();
  move |w, app| {
    let _ = me.update(app, |this, cx| f(this, w, cx));
  }
}

pub fn item(root: &mut Root, item: &Item, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Root>) {
  let mut m = Menu::default();
  let chat = root.snap.chat_for(item).cloned();
  let pinned = root.snap.pinned(item);
  let it = item.clone();
  m = m.item(if pinned { t("Unpin") } else { t("Pin") }, act(root, cx, move |this, _, cx| pin(this, &it, !pinned, cx)));

  // Sections.
  let it = item.clone();
  m = m.item(t("Move to new section"), act(root, cx, move |this, w, cx| {
    let it = it.clone();
    crate::root::dialogs::prompt(this, t("New section"), t("Section name"), "", w, cx, move |this, name, _, cx| {
      let rt = this.rt.clone();
      let it = it.clone();
      this.run(cx, async move {
        let s = store::sections::create(&rt.pool, &name).await?;
        move_to(&rt, &it, Some(&s.id)).await
      }, |this, _, cx| this.reload(cx));
    });
  }));
  let sections = root.snap.sections.clone();
  if !sections.is_empty() {
    m = m.section(t("Move to"));
    for s in sections {
      let it = item.clone();
      let sid = s.id.clone();
      m = m.item(s.name.clone(), act(root, cx, move |this, _, cx| {
        let rt = this.rt.clone();
        let (it, sid) = (it.clone(), sid.clone());
        this.run(cx, async move { move_to(&rt, &it, Some(&sid)).await }, |this, _, cx| this.reload(cx));
      }));
    }
    let it = item.clone();
    m = m.item(t("Unassigned"), act(root, cx, move |this, _, cx| {
      let rt = this.rt.clone();
      let it = it.clone();
      this.run(cx, async move { move_to(&rt, &it, None).await }, |this, _, cx| this.reload(cx));
    }));
    m = m.divider();
  }

  if let Some(c) = &chat {
    let (cid, unread) = (c.id.clone(), c.unread);
    m = m.item(if unread { t("Mark as Read") } else { t("Mark as Unread") }, act(root, cx, move |this, _, cx| {
      let rt = this.rt.clone();
      let cid = cid.clone();
      this.run(cx, async move { store::chats::set_unread(&rt.pool, &cid, !unread).await }, |this, _, cx| this.reload(cx));
    }));
  }
  match item {
    Item::Bot(id) => {
      let bot = root.snap.bot(id).cloned().unwrap_or_default();
      let it = item.clone();
      m = m.item(t("Rename Agent"), act(root, cx, move |this, w, cx| start_rename(this, &it, w, cx)));
      if let Some(c) = &chat {
        let cid = c.id.clone();
        m = m.item(t("Copy conversation ID"), act(root, cx, move |_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(cid.clone()))));
        let (cid, title) = (c.id.clone(), bot.name.clone());
        m = m.item(t("Export conversation…"), act(root, cx, move |this, _, cx| export(this, &cid, &title, cx)));
      }
      if bot.is_team() && bot.published {
        let bid = bot.id.clone();
        m = m.item(t("Copy link"), act(root, cx, move |this, _, cx| {
          let rt = this.rt.clone();
          let bid = bid.clone();
          this.run(cx, async move { agent::api::team::link(&rt, &bid).await }, |this, link, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(link));
            this.toast(t("Link copied"), cx);
          });
        }));
      }
      if !bot.primary_bot {
        let bid = bot.id.clone();
        m = m.item(t("Make primary Agent"), act(root, cx, move |this, _, cx| {
          let rt = this.rt.clone();
          let bid = bid.clone();
          this.run(cx, async move { store::bots::set_primary(&rt.pool, &bid).await }, |this, _, cx| this.reload(cx));
        }));
      }
      let bid = bot.id.clone();
      m = m.item(t("Duplicate"), act(root, cx, move |this, w, cx| {
        let rt = this.rt.clone();
        let bid = bid.clone();
        let win = w.window_handle();
        this.run(cx, async move { agent::api::bots::duplicate(&rt, &bid).await }, move |this, (_, c), cx| {
          this.reload(cx);
          let cid = c.id.clone();
          let _ = win.update(cx, |_, w, cx| {
            let _ = (w, cx);
          });
          this.pending_open(cid, cx);
        });
      }));
      if !bot.required {
        let (bid, hidden) = (bot.id.clone(), bot.hidden);
        m = m.item(if hidden { t("Show in sidebar") } else { t("Hide from sidebar") }, act(root, cx, move |this, _, cx| {
          let rt = this.rt.clone();
          let bid = bid.clone();
          this.run(cx, async move { store::bots::hide(&rt.pool, &bid, !hidden).await }, |this, _, cx| this.reload(cx));
        }));
      }
      m = m.divider();
      let (bid, name) = (bot.id.clone(), bot.name.clone());
      if bot.required {
        m = m.section(t("Required by your admin"));
      } else {
      m = m.danger(t("Delete"), act(root, cx, move |this, w, cx| {
        let bid = bid.clone();
        crate::root::dialogs::confirm(
          this,
          crate::i18n::tf("Delete {}?", &[&name]),
          t("This removes the Agent, its conversation, and its routines. Files on the computer and browser sign-ins stay."),
          t("Delete"),
          w,
          cx,
          move |this, _, cx| {
            let rt = this.rt.clone();
            let bid = bid.clone();
            if let Some(c) = this.snap.direct.get(&bid).map(|c| c.id.clone()) {
              this.history.forget(&c);
            }
            if this.active_bot().is_some_and(|b| b.id == bid) {
              this.active = None;
              this.pane = None;
              this.close_right(cx);
            }
            this.run(cx, async move { agent::api::bots::delete(&rt, &bid).await }, |this, _, cx| this.reload(cx));
          },
        );
      }));
      }
    }
    Item::Group(id) => {
      let it = item.clone();
      m = m.item(t("Rename chat"), act(root, cx, move |this, w, cx| start_rename(this, &it, w, cx)));
      let cid = id.clone();
      m = m.item(t("Copy conversation ID"), act(root, cx, move |_, _, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(cid.clone()))));
      let (eid, etitle) = (id.clone(), chat.as_ref().map(|c| c.title.clone()).unwrap_or_default());
      m = m.item(t("Export conversation…"), act(root, cx, move |this, _, cx| export(this, &eid, &etitle, cx)));
      let (gid, hidden) = (id.clone(), chat.as_ref().is_some_and(|c| c.hidden));
      m = m.item(if hidden { t("Show in sidebar") } else { t("Hide from sidebar") }, act(root, cx, move |this, _, cx| {
        let rt = this.rt.clone();
        let gid = gid.clone();
        this.run(cx, async move { store::chats::hide(&rt.pool, &gid, !hidden).await }, |this, _, cx| this.reload(cx));
      }));
      m = m.divider();
      let gid = id.clone();
      m = m.danger(t("Delete"), act(root, cx, move |this, w, cx| {
        let gid = gid.clone();
        crate::root::dialogs::confirm(this, t("Delete this group chat?"), t("The conversation is deleted. The Agents stay."), t("Delete"), w, cx, move |this, _, cx| {
          let rt = this.rt.clone();
          let gid = gid.clone();
          this.history.forget(&gid);
          if this.active.as_deref() == Some(gid.as_str()) {
            this.active = None;
            this.pane = None;
          }
          this.run(cx, async move { store::chats::delete(&rt.pool, &gid).await }, |this, _, cx| this.reload(cx));
        });
      }));
    }
  }
  show(root, m, at, window, cx);
}

pub fn section(root: &mut Root, id: &str, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Root>) {
  let name = root.snap.sections.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default();
  let (sid, sid2) = (id.to_string(), id.to_string());
  let m = Menu::default()
    .item(t("Rename"), act(root, cx, move |this, w, cx| {
      let sid = sid.clone();
      crate::root::dialogs::prompt(this, t("Rename section"), t("Section name"), &name, w, cx, move |this, new, _, cx| {
        let rt = this.rt.clone();
        let sid = sid.clone();
        this.run(cx, async move { store::sections::rename(&rt.pool, &sid, &new).await }, |this, _, cx| this.reload(cx));
      });
    }))
    .danger(t("Delete section"), act(root, cx, move |this, _, cx| {
      let rt = this.rt.clone();
      let sid = sid2.clone();
      this.run(cx, async move { store::sections::delete(&rt.pool, &sid).await }, |this, _, cx| this.reload(cx));
    }));
  show(root, m, at, window, cx);
}

async fn move_to(rt: &agent::Runtime, item: &Item, section: Option<&str>) -> anyhow::Result<()> {
  match item {
    Item::Bot(b) => store::bots::set_section(&rt.pool, b, section).await,
    Item::Group(g) => store::chats::set_section(&rt.pool, g, section).await,
  }
}

fn pin(root: &mut Root, item: &Item, on: bool, cx: &mut Context<Root>) {
  let rt = root.rt.clone();
  let item = item.clone();
  root.run(cx, async move {
    match &item {
      Item::Bot(b) => store::bots::pin(&rt.pool, b, on).await,
      Item::Group(g) => store::chats::pin(&rt.pool, g, on).await,
    }
  }, |this, _, cx| this.reload(cx));
}

/// Double-click (or Rename) edits the name in place; Enter commits.
pub fn start_rename(root: &mut Root, item: &Item, window: &mut Window, cx: &mut Context<Root>) {
  let current = match item {
    Item::Bot(b) => root.snap.bot(b).map(|b| b.name.clone()).unwrap_or_default(),
    Item::Group(g) => root.snap.groups.iter().find(|c| &c.id == g).map(|c| c.title.clone()).unwrap_or_default(),
  };
  let input = cx.new(|cx| TextInput::new(cx).value(&current).size(guise::Size::Xs));
  window.focus(&input.read(cx).focus_handle(), cx);
  input.update(cx, |i, cx| i.select_all(cx));
  let it = item.clone();
  let sub = cx.subscribe(&input, move |this: &mut Root, _, ev: &TextInputEvent, cx| {
    if let TextInputEvent::Submit(name) = ev {
      let name = name.trim().to_string();
      this.rename = None;
      if name.is_empty() {
        cx.notify();
        return;
      }
      let rt = this.rt.clone();
      let it = it.clone();
      this.run(cx, async move {
        match &it {
          Item::Bot(b) => agent::api::bots::rename(&rt, b, &name).await,
          Item::Group(g) => store::chats::rename(&rt.pool, g, &name).await,
        }
      }, |this, _, cx| this.reload(cx));
    }
  });
  root._subs.push(sub);
  root.rename = Some((item.clone(), input));
  cx.notify();
}

/// Save one conversation as Markdown (with a JSON copy beside it).
fn export(root: &mut Root, chat: &str, title: &str, cx: &mut Context<Root>) {
  let stem: String = title.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '.' }).collect();
  let name = format!("{}.md", stem.trim_matches('.'));
  let dir = std::env::var("HOME").map(|h| std::path::PathBuf::from(h).join("Downloads")).unwrap_or_default();
  let rx = cx.prompt_for_new_path(&dir, Some(&name));
  let (rt, chat) = (root.rt.clone(), chat.to_string());
  cx.spawn(async move |this, cx| {
    let Ok(Ok(Some(dest))) = rx.await else { return };
    let r = crate::tk::run(async move {
      let v = agent::api::export::chat_json(&rt, &chat).await?;
      std::fs::write(&dest, agent::api::export::markdown(&v))?;
      std::fs::write(dest.with_extension("json"), serde_json::to_string_pretty(&v)?)?;
      agent::audit::change(&rt, "user", "conversation.exported", &chat, "").await;
      anyhow::Ok(())
    })
    .await;
    let _ = this.update(cx, |this, cx| match r {
      Ok(()) => this.toast(t("Conversation exported."), cx),
      Err(e) => this.toast(e.to_string(), cx),
    });
  })
  .detach();
}
