//! Dictation: ⌘D or the mic button records; stopping transcribes the speech
//! into the composer for editing.

use super::Composer;
use crate::i18n::t;
use crate::theme::ink;
use crate::tk;
use gpui::prelude::*;
use gpui::{div, px, Context, Window};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Holding the shortcut longer than this makes release stop recording.
pub const HOLD: std::time::Duration = std::time::Duration::from_millis(600);

pub struct Dictation {
  pub recording: bool,
  pub level: Arc<AtomicU32>,
  pub started: std::time::Instant,
  stop: Option<tokio::sync::oneshot::Sender<()>>,
  samples: Arc<Mutex<Vec<f32>>>,
}

pub fn toggle(c: &mut Composer, window: &mut Window, cx: &mut Context<Composer>) {
  match c.dictation.take() {
    Some(mut d) if d.recording => {
      if let Some(s) = d.stop.take() {
        let _ = s.send(());
      }
      let samples = std::mem::take(&mut *d.samples.lock().expect("samples"));
      finish(c, samples, cx);
    }
    _ => start(c, window, cx),
  }
  cx.notify();
}

fn start(c: &mut Composer, _window: &mut Window, cx: &mut Context<Composer>) {
  let device = c.rt.settings().microphone;
  let mic = match voice::mic::Mic::start(&device) {
    Ok(m) => m,
    Err(e) => {
      let msg = if e.to_string().contains("No microphone") { t("No microphone found.").to_string() } else { t("Microphone access was denied. Allow it in System Settings → Privacy & Security → Microphone.").to_string() };
      let _ = c.pane.update(cx, |p, cx| p.toast(msg, cx));
      return;
    }
  };
  let level = Arc::new(AtomicU32::new(0));
  let samples = Arc::new(Mutex::new(Vec::new()));
  let (tx, mut rx) = tokio::sync::oneshot::channel();
  let (lv, sm) = (level.clone(), samples.clone());
  tk::spawn(async move {
    let mut mic = mic;
    loop {
      tokio::select! {
        chunk = mic.rx.recv() => {
          let Some(chunk) = chunk else { break };
          lv.store(voice::pcm::level(&chunk).to_bits(), Ordering::Relaxed);
          sm.lock().expect("samples").extend(chunk);
        }
        _ = &mut rx => break,
      }
    }
  });
  c.dictation = Some(Dictation { recording: true, level, started: std::time::Instant::now(), stop: Some(tx), samples });
  // Repaint the meter while recording.
  cx.spawn(async move |this, cx| loop {
    cx.background_executor().timer(std::time::Duration::from_millis(120)).await;
    let alive = this.update(cx, |this, cx| {
      cx.notify();
      this.dictation.as_ref().is_some_and(|d| d.recording)
    });
    if !matches!(alive, Ok(true)) {
      break;
    }
  })
  .detach();
}

fn finish(c: &mut Composer, samples: Vec<f32>, cx: &mut Context<Composer>) {
  if samples.len() < voice::RATE as usize / 4 {
    return;
  }
  let s = c.rt.settings();
  let (base, lang) = (String::new(), s.voice_language.clone());
  c.dictation = Some(Dictation {
    recording: false,
    level: Arc::new(AtomicU32::new(0)),
    started: std::time::Instant::now(),
    stop: None,
    samples: Arc::new(Mutex::new(Vec::new())),
  });
  cx.spawn(async move |this, cx| {
    let r = tk::run(async move {
      let key = config::secret::xai_key().ok_or_else(|| anyhow::anyhow!("Add your xAI API key in Settings to use dictation."))?;
      let wav = voice::pcm::wav(&voice::pcm::to_i16(&samples), voice::RATE);
      voice::stt::transcribe(&key, &base, wav, &lang).await
    })
    .await;
    let _ = this.update(cx, |this, cx| {
      this.dictation = None;
      match r {
        Ok(text) if !text.is_empty() => {
          let cur = this.text(cx);
          let sep = if cur.is_empty() || cur.ends_with(' ') { "" } else { " " };
          let new = format!("{cur}{sep}{text}");
          this.input.update(cx, |i, cx| i.set_text(&new, cx));
        }
        Ok(_) => {}
        Err(e) => {
          let msg = e.to_string();
          let _ = this.pane.update(cx, |p, cx| p.toast(msg, cx));
        }
      }
      cx.notify();
    });
  })
  .detach();
}

pub fn render(d: &Dictation, cx: &mut Context<Composer>) -> impl IntoElement {
  let ink = ink(cx);
  if !d.recording {
    return div().text_size(px(12.0)).text_color(ink.dimmed).child(t("Transcribing…"));
  }
  let level = f32::from_bits(d.level.load(Ordering::Relaxed));
  let secs = d.started.elapsed().as_secs();
  div()
    .flex()
    .items_center()
    .gap(px(8.0))
    .text_size(px(12.0))
    .child(div().size(px(8.0)).rounded_full().bg(ink.danger))
    .child(format!("{} {}:{:02}", t("Listening"), secs / 60, secs % 60))
    .child(div().h(px(4.0)).w(px(120.0)).rounded_full().bg(ink.hover).child(div().h_full().rounded_full().bg(ink.primary).w(px((level * 600.0).min(120.0)))))
}
