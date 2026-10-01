//! The Marketplace (⇧⌘M): plugins (MCP connectors) and packaged skills to
//! add, and Your plugins — what's installed, its accounts and tools, and
//! your private skills.

pub mod detail;
pub mod custom;

use crate::i18n::t;
use crate::root::Root;
use crate::theme::ink;
use crate::tk;
use agent::Runtime;
use gpui::prelude::*;
use gpui::{div, px, Context, Entity, SharedString, WeakEntity, Window};
use guise::{Badge, Button, IconName, SegmentedControl, SegmentedControlEvent, Size, TextInput, Variant};
use std::collections::HashMap;
use store::{Account, Plugin, Skill};

pub struct Market {
  pub rt: Runtime,
  pub root: WeakEntity<Root>,
  pub tabs: Entity<SegmentedControl>,
  pub tab: usize,
  pub search: Entity<TextInput>,
  pub installed: Vec<Plugin>,
  pub accounts: HashMap<String, Vec<Account>>,
  pub skills: Vec<Skill>,
  pub open: Option<String>,
  pub detail: Option<detail::State>,
  pub custom: Option<custom::Form>,
  pub status: Option<String>,
  pub _subs: Vec<gpui::Subscription>,
}

pub fn open(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
  let rt = root.rt.clone();
  let weak = cx.entity().downgrade();
  let view = cx.new(|cx| {
    let tabs = cx.new(|cx| SegmentedControl::new(cx).data([t("Plugins"), t("Skills"), t("Your plugins")]).selected(0).size(Size::Sm));
    let search = cx.new(|cx| TextInput::new(cx).placeholder(t("Search plugins and skills")).size(Size::Sm));
    window.focus(&search.read(cx).focus_handle(), cx);
    let s1 = cx.subscribe(&tabs, |this: &mut Market, _, ev: &SegmentedControlEvent, cx| {
      this.tab = ev.0;
      this.open = None;
      cx.notify();
    });
    let s2 = cx.observe(&search, |_, _, cx| cx.notify());
    let mut m = Market { rt, root: weak, tabs, tab: 0, search, installed: Vec::new(), accounts: HashMap::new(), skills: Vec::new(), open: None, detail: None, custom: None, status: None, _subs: vec![s1, s2] };
    m.load(cx);
    m
  });
  root.market = Some(view.clone());
  root.set_modal(view, cx);
}

impl Market {
  pub fn load(&mut self, cx: &mut Context<Self>) {
    let rt = self.rt.clone();
    cx.spawn(async move |this, cx| {
      let r = tk::run(async move {
        let installed = store::plugins::all(&rt.pool).await?;
        let mut accounts = HashMap::new();
        for p in &installed {
          accounts.insert(p.id.clone(), store::plugins::accounts(&rt.pool, &p.id).await?);
        }
        Ok((installed, accounts, store::skills::all(&rt.pool).await?))
      })
      .await;
      let _ = this.update(cx, |this, cx| {
        if let Ok((i, a, s)) = r {
          this.installed = i;
          this.accounts = a;
          this.skills = s;
        }
        cx.notify();
      });
    })
    .detach();
  }

  pub fn run<F, T>(&mut self, cx: &mut Context<Self>, f: F, then: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static)
  where
    F: std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
    T: Send + 'static,
  {
    cx.spawn(async move |this, cx| {
      let r = tk::run(f).await;
      let _ = this.update(cx, |this, cx| match r {
        Ok(v) => then(this, v, cx),
        Err(e) => {
          this.status = Some(e.to_string());
          cx.notify();
        }
      });
    })
    .detach();
  }

  pub fn on_plugins(&mut self, cx: &mut Context<Self>) {
    self.load(cx);
  }

  fn status_of(&self, catalog: &str) -> Option<(String, String)> {
    self.installed.iter().find(|p| p.catalog == catalog).map(|p| (p.id.clone(), p.status.clone()))
  }
}

pub fn status_badge(status: &str) -> Badge {
  let (label, color) = match status {
    "connected" => (t("Connected"), guise::ColorName::Green),
    "needs auth" => (t("Needs auth"), guise::ColorName::Yellow),
    "waiting for authorization" => (t("Waiting for authorization"), guise::ColorName::Violet),
    "disconnected" => (t("Disconnected"), guise::ColorName::Red),
    _ => (t("Added"), guise::ColorName::Gray),
  };
  Badge::new(label).size(Size::Xs).color(color)
}

impl Render for Market {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let ink = ink(cx);
    let root = self.root.clone();
    let q = self.search.read(cx).text().to_lowercase();
    let hit = |s: &str| q.is_empty() || s.to_lowercase().contains(&q);
    let body: gpui::AnyElement = if let Some(id) = self.open.clone() {
      detail::render(self, &id, window, cx)
    } else if let Some(f) = &self.custom {
      custom::render(self, f.clone(), cx)
    } else {
      let mut grid = div().flex().flex_wrap().gap(px(10.0));
      match self.tab {
        0 => {
          for e in agent::catalog::PLUGINS.iter().filter(|e| hit(e.name) || hit(e.description) || hit(e.category)) {
            let status = self.status_of(e.id);
            let blocked = self.rt.policy().blocks(Some(e.id), e.name);
            let id = e.id;
            grid = grid.child(
              card(&ink)
                .id(SharedString::from(format!("pl-{}", e.id)))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                  if blocked {
                    return;
                  }
                  match this.status_of(id) {
                    Some((pid, _)) => this.open = Some(pid),
                    None => {
                      let rt = this.rt.clone();
                      this.run(cx, async move { agent::api::connect::add(&rt, id).await }, |this, p, cx| {
                        this.open = Some(p.id);
                        this.load(cx);
                      });
                    }
                  }
                  cx.notify();
                }))
                .child(div().flex().items_center().gap(px(8.0)).child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(e.name)).child(div().flex_1()).child(match &status {
                  _ if blocked => guise::Badge::new(t("Disabled by your admin")).size(Size::Xs).variant(Variant::Light).color(guise::ColorName::Gray).into_any_element(),
                  Some((_, s)) => status_badge(s).into_any_element(),
                  None => Button::new(SharedString::from(format!("add-{}", e.id)), t("Add")).size(Size::Xs).variant(Variant::Light).into_any_element(),
                }))
                .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t(e.description)))
                .child(div().text_size(px(11.0)).text_color(ink.dimmed).child(format!("{} · {}", t(e.category), if e.kind == "command" { t("Runs on your computer") } else { t("Remote") }))),
            );
          }
          grid = grid.child(
            card(&ink)
              .id("custom-mcp")
              .cursor_pointer()
              .border_dashed()
              .on_click(cx.listener(|this, _, w, cx| {
                this.custom = Some(custom::Form::new(w, cx));
                cx.notify();
              }))
              .child(div().flex().items_center().gap(px(6.0)).child(guise::Icon::new(IconName::Plus).size(Size::Sm)).child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(t("Custom MCP server"))))
              .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t("Remote HTTPS (with optional sign-in) or a local command."))),
          );
        }
        1 => {
          for s in agent::catalog::SKILLS.iter().filter(|s| hit(s.name) || hit(s.description)) {
            let added = self.skills.iter().any(|x| x.name.eq_ignore_ascii_case(s.name));
            let name = s.name;
            grid = grid.child(
              card(&ink)
                .child(div().flex().items_center().child(div().flex_1().font_weight(gpui::FontWeight::SEMIBOLD).child(format!("/{}", s.name))).child(if added {
                  Badge::new(t("Added")).size(Size::Xs).color(guise::ColorName::Green).into_any_element()
                } else {
                  Button::new(SharedString::from(format!("adds-{}", s.name)), t("Add")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(move |this, _, _, cx| {
                    let rt = this.rt.clone();
                    this.run(cx, async move { agent::api::bots::add_packaged_skill(&rt, name).await }, |this, s, cx| {
                      this.status = Some(crate::i18n::tf("Added /{}. Switch it on for a Bot in its Skills tab.", &[&s.name]));
                      this.load(cx);
                    });
                  })).into_any_element()
                }))
                .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(t(s.description))),
            );
          }
        }
        _ => {
          let mut col = div().flex().flex_col().gap(px(6.0)).w_full();
          col = col.child(div().text_size(px(11.0)).font_weight(gpui::FontWeight::SEMIBOLD).text_color(ink.dimmed).child(t("INSTALLED")));
          if self.installed.is_empty() {
            col = col.child(div().text_size(px(13.0)).text_color(ink.dimmed).child(t("No plugins yet.")));
          }
          for p in self.installed.clone() {
            let id = p.id.clone();
            let n = self.accounts.get(&p.id).map(|a| a.len()).unwrap_or(0);
            col = col.child(
              div()
                .id(SharedString::from(format!("inst-{}", p.id)))
                .flex()
                .items_center()
                .gap(px(8.0))
                .p(px(10.0))
                .rounded(px(8.0))
                .bg(ink.surface)
                .cursor_pointer()
                .hover(|s| s.bg(ink.hover))
                .on_click(cx.listener(move |this, _, _, cx| {
                  this.open = Some(id.clone());
                  cx.notify();
                }))
                .child(div().flex_1().child(p.name.clone()))
                .child(div().text_size(px(12.0)).text_color(ink.dimmed).child(crate::i18n::tf("{} accounts", &[&n.to_string()])))
                .child(status_badge(&p.status)),
            );
          }
          col = col.child(div().pt(px(12.0)).text_size(px(11.0)).font_weight(gpui::FontWeight::SEMIBOLD).text_color(ink.dimmed).child(t("PRIVATE SKILLS")));
          for s in self.skills.clone() {
            let sid = s.id.clone();
            col = col.child(
              div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .p(px(8.0))
                .rounded(px(8.0))
                .bg(ink.surface)
                .child(div().flex_1().flex().flex_col().child(format!("/{}", s.name)).child(div().text_size(px(12.0)).text_color(ink.dimmed).truncate().child(SharedString::from(s.description.clone()))))
                .child(Button::new(SharedString::from(format!("dels-{}", s.id)), t("Remove")).size(Size::Xs).variant(Variant::Subtle).color(guise::ColorName::Red).on_click(cx.listener(move |this, _, _, cx| {
                  let rt = this.rt.clone();
                  let sid = sid.clone();
                  this.run(cx, async move { store::skills::delete(&rt.pool, &sid).await }, |this, _, cx| this.load(cx));
                }))),
            );
          }
          if self.skills.is_empty() {
            col = col.child(div().text_size(px(13.0)).text_color(ink.dimmed).child(t("No skills yet.")));
          }
          grid = grid.child(col);
        }
      }
      grid.into_any_element()
    };
    let mut header = div().flex().items_center().gap(px(10.0)).child(self.tabs.clone()).child(div().flex_1().child(self.search.clone()));
    if self.open.is_some() || self.custom.is_some() {
      header = div().flex().items_center().child(Button::new("back", t("Back")).size(Size::Xs).variant(Variant::Subtle).left_section(guise::Icon::new(IconName::ChevronLeft).size(Size::Xs)).on_click(cx.listener(|this, _, _, cx| {
        this.open = None;
        this.custom = None;
        this.status = None;
        cx.notify();
      })));
    }
    div().absolute().top_0().left_0().size_full().child(
      guise::Modal::new()
        .title(t("Marketplace"))
        .width(840.0)
        .on_close(move |_, w, cx| {
          let _ = root.update(cx, |r, cx| r.close_modal(w, cx));
        })
        .child(
          div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .h(px(560.0))
            .child(header)
            .child(div().id("market-body").flex_1().min_h_0().overflow_y_scroll().child(body))
            .when_some(self.status.clone(), |d, s| d.child(div().text_size(px(12.0)).text_color(ink.primary).child(s))),
        ),
    )
  }
}

fn card(ink: &crate::theme::Ink) -> gpui::Div {
  div().w(px(248.0)).flex().flex_col().gap(px(6.0)).p(px(12.0)).rounded(px(10.0)).border_1().border_color(ink.border).bg(ink.surface)
}
