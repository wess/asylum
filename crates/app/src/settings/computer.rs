//! Settings → Computer: the shared computer's state, disk, backups, and its
//! lifecycle (update, recover, reset), plus local execution.

use super::{heading, row, Dialog};
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString, Window};
use guise::{Button, Size, Variant};

pub fn gb(n: u64) -> String {
  format!("{:.1} GB", n as f64 / 1_073_741_824.0)
}

fn op(d: &mut Dialog, label: &'static str, cx: &mut Context<Dialog>, f: fn(agent::Runtime) -> futures::future::BoxFuture<'static, anyhow::Result<()>>) {
  d.status = Some(format!("{}…", t(label)));
  let rt = d.rt.clone();
  cx.spawn(async move |this, cx| {
    let r = crate::tk::run(f(rt)).await;
    let _ = this.update(cx, |this, cx| {
      this.status = Some(match r {
        Ok(()) => t("Done.").to_string(),
        Err(e) => e.to_string(),
      });
      this.refresh(cx);
    });
  })
  .detach();
}

pub fn render(d: &mut Dialog, window: &mut Window, cx: &mut Context<Dialog>) -> AnyElement {
  let ink = ink(cx);
  let _ = window;
  let disk = d.disk.clone().unwrap_or_default();
  let chrome = computer::browser::find_chrome().map(|p| p.display().to_string()).unwrap_or_else(|| t("No Chromium browser found — install Chrome, Edge, or Chromium.").to_string());
  let ws = d.rt.computer.workspace().display().to_string();
  let ws2 = ws.clone();
  let mut col = div()
    .flex()
    .flex_col()
    .child(heading(t("Computers"), cx))
    .child(row(t("This Mac"), Some(t("The computer every Agent shares: one workspace, one browser profile, a screen per Agent.")), div().child(d.local.clone()).w(px(200.0)), cx))
    .child(row(t("Workspace"), Some(&ws), Button::new("reveal-ws", t("Show in Finder")).size(Size::Xs).variant(Variant::Light).on_click(move |_, _, cx| cx.reveal_path(std::path::Path::new(&ws2))), cx))
    .child(row(t("Browser"), Some(&chrome), div(), cx))
    .child(row(t("Route traffic through this computer"), Some(t("Agents already browse from this Mac's network.")), guise::Switch::new("egress").checked(true).disabled(true), cx))
    .child(heading(t("Disk"), cx))
    .child(row(t("Workspace files"), None, div().child(gb(disk.workspace)), cx))
    .child(row(t("Browser data"), None, div().child(gb(disk.browser)), cx))
    .child(row(t("Backups"), Some(&format!("{} {}", d.backups, t("kept (daily, one week)"))), div().child(gb(disk.backups)), cx))
    .child(row(t("Free space"), None, div().child(disk.free.map(gb).unwrap_or_default()), cx))
    .child(heading(t("Maintenance"), cx))
    .child(row(t("Back up now"), None, Button::new("backup", t("Back up")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, _, cx| op(this, "Backing up", cx, |rt| Box::pin(async move { agent::api::computer::backup(&rt).await.map(|_| ()) })))), cx))
    .child(row(t("Update Asylum's Computer"), Some(t("A software update keeps everything. A computer update also clears caches and temporary files but keeps files and sign-ins.")), guise::Group::new().gap(Size::Xs)
      .child(Button::new("sw-update", t("Software update")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, _, cx| op(this, "Updating", cx, |rt| Box::pin(async move { agent::api::computer::update(&rt, false).await })))))
      .child(Button::new("full-update", t("Computer update")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(|this, _, _, cx| op(this, "Updating", cx, |rt| Box::pin(async move { agent::api::computer::update(&rt, true).await })))))
      .child(Button::new("later-update", if d.update_at.is_some() { t("Cancel scheduled update") } else { t("Update tonight") }).size(Size::Xs).variant(Variant::Subtle).on_click(cx.listener(|this, _, _, cx| {
        let rt = this.rt.clone();
        let on = this.update_at.is_none();
        cx.spawn(async move |this, cx| {
          let r = crate::tk::run(async move { agent::api::computer::schedule_update(&rt, on).await }).await;
          let _ = this.update(cx, |this, cx| {
            if let Ok(at) = r {
              this.update_at = at;
            }
            cx.notify();
          });
        })
        .detach();
      }))), cx))
    .when_some(d.update_at, |c, at| c.child(div().text_size(px(12.0)).text_color(ink.dimmed).pb(px(6.0)).child(crate::i18n::tf("Computer update scheduled for {}. It waits until no Agent is working.", &[&chrono::DateTime::from_timestamp_millis(at).map(|d| d.with_timezone(&chrono::Local).format("%a %-I:%M %p").to_string()).unwrap_or_default()]))))
    .child(row(t("Recover computer"), Some(t("Rebuild the parts that can break, keeping your files. Use it when the computer can't be reached.")), Button::new("recover", t("Recover")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(|this, _, _, cx| op(this, "Recovering", cx, |rt| Box::pin(async move { agent::api::computer::recover(&rt).await })))), cx))
    .child(row(t("Recreate computer"), Some(t("A fresh computer on the same disk: Agents pause at a safe point and resume on it. Files, sign-ins, and backups stay.")), Button::new("recreate", t("Recreate")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(|this, _, _, cx| op(this, "Recreating", cx, |rt| Box::pin(async move { agent::api::computer::recreate(&rt).await })))), cx))
    .child(row(t("Hibernate when idle"), Some(t("Stop the browser after 30 idle minutes to save memory. It wakes when an Agent needs it.")), guise::Switch::new("hibernate").checked(d.rt.settings().hibernate_minutes > 0).color(guise::ColorName::Violet).on_change({
      let rt = d.rt.clone();
      let on = d.rt.settings().hibernate_minutes == 0;
      move |_, _, cx| {
        crate::settings::save(&rt, |s| s.hibernate_minutes = if on { 30 } else { 0 });
        cx.refresh_windows();
      }
    }), cx))
    .child(row(t("Reset Asylum's Computer"), Some(t("Last resort: restore the workspace from the latest backup.")), Button::new("reset", t("Reset")).size(Size::Xs).color(guise::ColorName::Red).variant(Variant::Light).on_click(cx.listener(|this, _, _, cx| op(this, "Resetting", cx, |rt| Box::pin(async move { agent::api::computer::reset(&rt).await })))), cx))
    .child(row(t("Stop computer"), Some(t("Shut it down now. It starts again when an Agent needs it.")), Button::new("stop-computer", t("Stop")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(|this, _, _, cx| op(this, "Stopping", cx, |rt| Box::pin(async move { agent::api::computer::stop(&rt).await })))), cx))
    .child(row(t("Delete computer and data"), Some(t("Erase workspace files, browser sign-ins, and backups. Agents and conversations stay. This can't be undone.")), Button::new("delete-computer", t("Delete")).size(Size::Xs).color(guise::ColorName::Red).variant(Variant::Light).on_click(cx.listener(|this, _, w, cx| {
      let Some(root) = this.root.upgrade() else { return };
      root.update(cx, |r, cx| {
        crate::root::dialogs::confirm(r, t("Delete computer and data?"), t("Workspace files, browser sign-ins, and backups are erased. This can't be undone."), t("Delete"), w, cx, move |r, _, cx| {
          let rt = r.rt.clone();
          r.run(cx, async move { agent::api::computer::delete_all(&rt).await }, |r, _, cx| r.toast(t("Computer deleted. It starts fresh."), cx));
        });
      });
    })), cx))
    .child(row(t("Sign out of every site"), Some(t("Clear the shared browser's cookies.")), Button::new("signout", t("Sign out")).size(Size::Xs).variant(Variant::Default).on_click(cx.listener(|this, _, _, cx| op(this, "Signing out", cx, |rt| Box::pin(async move { agent::api::computer::sign_out_sites(&rt).await })))), cx));
  if let Some(s) = &d.status {
    col = col.child(div().pt(px(8.0)).text_size(px(12.0)).text_color(ink.primary).child(SharedString::from(s.clone())));
  }
  col.into_any_element()
}
