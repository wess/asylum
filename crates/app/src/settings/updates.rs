//! Updates: the installed version, checking GitHub releases, and automatic
//! updates.

use super::{heading, row, switch, Dialog};
use crate::i18n::t;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString};
use guise::{Button, Size, Variant};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO: &str = "wess/asylum";

/// The latest release tag, or None when there is none or it can't be read.
pub async fn latest() -> anyhow::Result<Option<String>> {
  let res = reqwest_get(&format!("https://api.github.com/repos/{REPO}/releases/latest")).await?;
  Ok(res["tag_name"].as_str().map(|s| s.trim_start_matches('v').to_string()))
}

async fn reqwest_get(url: &str) -> anyhow::Result<serde_json::Value> {
  let out = tokio::process::Command::new("curl").args(["-fsSL", "-H", "User-Agent: asylum", url]).output().await?;
  if !out.status.success() {
    anyhow::bail!("no published release yet");
  }
  Ok(serde_json::from_slice(&out.stdout)?)
}

pub fn newer(latest: &str, current: &str) -> bool {
  let parse = |v: &str| v.split('.').map(|x| x.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
  parse(latest) > parse(current)
}

pub fn render(d: &mut Dialog, cx: &mut Context<Dialog>) -> AnyElement {
  let ink = ink(cx);
  let s = d.rt.settings();
  let status = d.update.clone().unwrap_or_default();
  div()
    .flex()
    .flex_col()
    .child(heading(t("Asylum Updates"), cx))
    .child(row(t("Version"), None, div().child(VERSION), cx))
    .child(row(t("Check for Updates"), if status.is_empty() { None } else { Some(&status) }, Button::new("check", t("Check")).size(Size::Xs).variant(Variant::Light).on_click(cx.listener(|this, _, _, cx| {
      this.update = Some(t("Checking…").into());
      cx.spawn(async move |this, cx| {
        let r = crate::tk::run(latest()).await;
        let _ = this.update(cx, |this, cx| {
          this.update = Some(match r {
            Ok(Some(v)) if newer(&v, VERSION) => crate::i18n::tf("Version {} is available. Download it from the releases page.", &[&v]),
            Ok(_) => t("You're up to date.").into(),
            Err(e) => e.to_string(),
          });
          cx.notify();
        });
      })
      .detach();
    })), cx))
    .child(row(t("Automatic Updates"), None, switch("auto-updates", s.automatic_updates, &d.rt, |s, v| s.automatic_updates = v), cx))
    .child(row(t("Releases"), None, Button::new("releases", t("Open releases")).size(Size::Xs).variant(Variant::Default).on_click(|_, _, cx| cx.open_url(&format!("https://github.com/{REPO}/releases"))), cx))
    .child(div().pt(px(10.0)).text_size(px(12.0)).text_color(ink.dimmed).child(SharedString::from(t("Computer updates, recovery, and reset are under Computer."))))
    .into_any_element()
}

#[cfg(test)]
#[path = "../../tests/updates.rs"]
mod tests;
