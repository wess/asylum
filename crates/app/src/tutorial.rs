//! The bundled getting-started video and its accessible local player.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, Result};
use gpui::prelude::*;
use gpui::{div, px, Entity, FocusHandle, IntoElement, KeyDownEvent, Window};
use guise::prelude::*;

use crate::state::Root;

const VIDEO: &[u8] = include_bytes!("../../../site/public/videos/overview.mp4");
const POSTER: &[u8] = include_bytes!("../../../site/public/videos/overview.png");
const CAPTIONS: &str = include_str!("../../../site/public/videos/overview.vtt");
const CHAPTERS: &str = include_str!("../../../site/public/videos/overviewchapters.vtt");
const TRANSCRIPT: &str = include_str!("../../../site/public/videos/transcripts/overview.md");

const CHAPTER_LINKS: &[(&str, &str, f64)] = &[
    ("Start here", "0:00", 0.0),
    ("Explain like I am 5", "0:19", 19.407),
    ("Your pace", "1:25", 85.140),
    ("First launch", "1:49", 109.140),
    ("Your first task", "2:40", 160.833),
    ("What isolation means", "3:55", 235.105),
    ("Watch the fleet", "4:17", 257.351),
    ("Review the evidence", "5:07", 307.262),
    ("Finish safely", "6:21", 381.844),
    ("Add durable context", "6:48", 408.653),
    ("After the first win", "7:14", 434.184),
    ("Good defaults", "7:46", 466.252),
    ("Your next five clicks", "8:13", 493.208),
];

/// Open the local player. The media is extracted beside Asylum's database on
/// first use, so packaged builds need no network connection or extra resource
/// lookup rules.
pub fn open(root: &mut Root, window: &mut Window, cx: &mut gpui::Context<Root>) {
    if root.tutorial.is_some() {
        return;
    }
    let directory = match ensure_assets() {
        Ok(directory) => directory,
        Err(error) => {
            root.push_error(
                "Could not open the getting-started video",
                error.to_string(),
            );
            cx.notify();
            return;
        }
    };
    let view = cx.new(|cx| {
        guise::WebView::new(cx)
            .serve(directory, "index.html")
            .bordered(false)
    });
    cx.subscribe(&view, |root, _view, event: &guise::WebViewEvent, cx| {
        if matches!(event, guise::WebViewEvent::Message(message) if message.as_ref() == "close") {
            close(root, cx);
        }
    })
    .detach();
    let focus = cx.focus_handle();
    root.tutorial_previous_focus = window.focused(cx);
    root.tutorial_restore_focus = None;
    window.focus(&focus, cx);
    root.tutorial = Some(view);
    root.tutorial_focus = Some(focus);
    cx.notify();
}

pub fn close(root: &mut Root, cx: &mut gpui::Context<Root>) {
    if let Some(view) = root.tutorial.take() {
        view.update(cx, |view, _cx| view.set_visible(false));
        root.tutorial_closing = Some(view);
        let timer = cx.background_executor().timer(Duration::from_millis(50));
        cx.spawn(async move |root, cx| {
            timer.await;
            let _ = root.update(cx, |root, cx| {
                if let Some(view) = root.tutorial_closing.take() {
                    view.update(cx, |view, _cx| view.set_visible(false));
                }
                cx.notify();
            });
        })
        .detach();
    }
    root.tutorial_focus = None;
    root.tutorial_restore_focus = root.tutorial_previous_focus.take();
    cx.notify();
}

pub fn modal(
    view: Entity<guise::WebView>,
    focus: FocusHandle,
    handle: Entity<Root>,
    window: &Window,
) -> impl IntoElement {
    let viewport = window.viewport_size();
    let width = (f32::from(viewport.width) - 48.0).clamp(300.0, 1120.0);
    let height = (f32::from(viewport.height) - 140.0)
        .min(width * 0.73)
        .max(280.0);
    let dismiss = handle.clone();
    div()
        .id("getting-started-overlay")
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .size_full()
        .track_focus(&focus)
        .on_key_down(move |event: &KeyDownEvent, _window, cx| {
            if event.keystroke.key.as_str() == "escape" {
                dismiss.update(cx, |root, cx| close(root, cx));
                cx.stop_propagation();
            }
        })
        .child(
            Modal::new()
                .title("Asylum: Start Here · 8 min 41 sec")
                .width(width)
                .padding(Size::Sm)
                .on_close(move |_, _, cx| {
                    handle.update(cx, |root, cx| close(root, cx));
                })
                .child(
                    div()
                        .id("getting-started-video")
                        .w_full()
                        .h(px(height))
                        .overflow_hidden()
                        .rounded(px(6.0))
                        .child(view),
                ),
        )
}

fn ensure_assets() -> Result<PathBuf> {
    let html = player_html();
    let parent = Root::db_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir);
    let directory = parent.join("tutorial").join(format!(
        "overview-{}-{:016x}",
        env!("CARGO_PKG_VERSION"),
        fingerprint(&[
            VIDEO,
            POSTER,
            CAPTIONS.as_bytes(),
            CHAPTERS.as_bytes(),
            html.as_bytes(),
        ])
    ));
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("create {}", directory.display()))?;
    write_if_changed(&directory.join("overview.mp4"), VIDEO)?;
    write_if_changed(&directory.join("overview.png"), POSTER)?;
    write_if_changed(&directory.join("overview.vtt"), CAPTIONS.as_bytes())?;
    write_if_changed(&directory.join("overviewchapters.vtt"), CHAPTERS.as_bytes())?;
    write_if_changed(&directory.join("index.html"), html.as_bytes())?;
    Ok(directory)
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<()> {
    if std::fs::metadata(path)
        .map(|metadata| metadata.len() == bytes.len() as u64)
        .unwrap_or(false)
    {
        return Ok(());
    }
    std::fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}

fn fingerprint(parts: &[&[u8]]) -> u64 {
    parts
        .iter()
        .flat_map(|part| part.iter())
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

fn player_html() -> String {
    let chapters = CHAPTER_LINKS
        .iter()
        .map(|(title, time, seconds)| {
            format!(
                r#"<button type="button" data-time="{seconds}"><span>{}</span><time>{time}</time></button>"#,
                html_escape(title)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Asylum: Start Here</title>
<style>
:root {{ color-scheme: dark; --bg:#05070b; --surface:#0d121a; --line:#243342; --text:#f1f6fa; --muted:#a9bac7; --accent:#33d6ff; --focus:#80f0c0; }}
* {{ box-sizing:border-box; }}
html {{ background:var(--bg); scroll-behavior:smooth; }}
body {{ margin:0; color:var(--text); background:var(--bg); font:15px/1.55 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; }}
::selection {{ color:#001019; background:var(--accent); }}
:focus-visible {{ outline:3px solid var(--focus); outline-offset:3px; }}
::-webkit-scrollbar {{ width:12px; }}
::-webkit-scrollbar-track {{ background:var(--bg); }}
::-webkit-scrollbar-thumb {{ background:#324657; border:3px solid var(--bg); border-radius:8px; }}
main {{ width:100%; max-width:1120px; margin:0 auto; }}
.player {{ position:relative; width:100%; aspect-ratio:16/9; background:#000; }}
video {{ display:block; width:100%; height:100%; background:#000; }}
.loading {{ position:absolute; inset:auto 18px 18px; width:max-content; max-width:calc(100% - 36px); padding:8px 12px; border:1px solid var(--line); border-radius:6px; color:var(--muted); background:rgba(5,7,11,.92); }}
.loading[hidden] {{ display:none; }}
.content {{ display:grid; grid-template-columns:minmax(230px,.8fr) minmax(0,1.6fr); grid-template-areas:"chapters copy"; gap:28px; padding:24px 28px 40px; border-top:1px solid var(--line); }}
.copy {{ grid-area:copy; min-width:0; }}
h1 {{ margin:0 0 8px; font-size:clamp(26px,4vw,40px); line-height:1.08; letter-spacing:-.025em; }}
.intro {{ margin:0 0 18px; max-width:70ch; color:var(--muted); }}
.hint {{ margin:16px 0 0; color:var(--muted); font-size:13px; }}
.chapters {{ grid-area:chapters; display:flex; flex-direction:column; gap:2px; }}
.chapters h2, details h2 {{ margin:0 0 10px; font-size:15px; }}
.chapters button {{ display:flex; justify-content:space-between; gap:12px; width:100%; padding:8px 10px; border:0; border-radius:5px; color:var(--muted); background:transparent; font:inherit; text-align:left; cursor:pointer; }}
.chapters button:hover {{ color:var(--text); background:#121c26; }}
.chapters time {{ flex:none; color:#79cbe9; font-variant-numeric:tabular-nums; }}
.close {{ width:max-content; margin:0 0 18px; padding:7px 11px; border:1px solid var(--line); border-radius:5px; color:var(--text); background:#121c26; font:inherit; font-weight:700; cursor:pointer; }}
.close:hover {{ border-color:#4c687e; background:#192632; }}
details {{ min-width:0; }}
summary {{ width:max-content; max-width:100%; padding:7px 0; color:var(--accent); font-weight:700; cursor:pointer; text-underline-offset:3px; }}
.transcript {{ max-width:72ch; padding-top:10px; }}
.transcript h2 {{ margin:26px 0 8px; font-size:18px; line-height:1.25; }}
.transcript p {{ margin:0 0 14px; color:#cedae3; }}
@media (max-width:720px) {{ .content {{ grid-template-columns:1fr; grid-template-areas:"copy" "chapters"; padding:20px; }} .chapters {{ max-height:240px; overflow:auto; }} }}
@media (prefers-reduced-motion:reduce) {{ html {{ scroll-behavior:auto; }} }}
</style>
</head>
<body>
<main>
  <section class="player" aria-label="Getting-started video player">
    <video id="video" controls playsinline preload="metadata" poster="overview.png" aria-label="Asylum: Start Here">
      Your browser cannot play this video. The complete transcript is available below.
    </video>
    <p class="loading" id="status" role="status" aria-live="polite">Preparing the offline video…</p>
  </section>
  <section class="content">
    <div class="copy">
      <h1>Start with the simple version</h1>
      <p class="intro">One task, separate workspaces, visible evidence, and one deliberate winner. The tour starts there, then completes a safe run from project selection through review and merge.</p>
      <button class="close" id="close" type="button">Close tour</button>
      <details>
        <summary>Read the full transcript</summary>
        <article class="transcript">{transcript}</article>
      </details>
    </div>
    <nav class="chapters" aria-label="Video chapters">
      <h2>Jump to a chapter</h2>
      {chapters}
      <p class="hint">Captions are enabled by default. Press Escape to close.</p>
    </nav>
  </section>
</main>
<script>
const video = document.querySelector('#video');
const status = document.querySelector('#status');
let urls = [];
async function prepare() {{
  status.hidden = false;
  status.textContent = 'Preparing the offline video…';
  try {{
    const [media, captions, chapters] = await Promise.all([
      fetch('overview.mp4').then(response => {{ if (!response.ok) throw new Error('video'); return response.arrayBuffer(); }}),
      fetch('overview.vtt').then(response => response.text()),
      fetch('overviewchapters.vtt').then(response => response.text())
    ]);
    const source = URL.createObjectURL(new Blob([media], {{ type:'video/mp4' }}));
    const captionSource = URL.createObjectURL(new Blob([captions], {{ type:'text/vtt' }}));
    const chapterSource = URL.createObjectURL(new Blob([chapters], {{ type:'text/vtt' }}));
    urls = [source, captionSource, chapterSource];
    video.src = source;
    for (const [kind, label, src, enabled] of [['captions','English',captionSource,true],['chapters','Chapters',chapterSource,false]]) {{
      const track = document.createElement('track');
      track.kind = kind; track.label = label; track.srclang = 'en'; track.src = src; track.default = enabled;
      video.appendChild(track);
    }}
    video.addEventListener('loadedmetadata', () => {{
      if (video.textTracks[0]) video.textTracks[0].mode = 'showing';
      status.hidden = true;
    }}, {{ once:true }});
    video.load();
  }} catch (_) {{
    status.textContent = 'The video could not be prepared. Close this window and try again.';
  }}
}}
document.querySelectorAll('[data-time]').forEach(button => button.addEventListener('click', () => {{
  video.currentTime = Number(button.dataset.time); video.focus();
}}));
document.querySelector('#close').addEventListener('click', () => {{
  if (window.ipc) window.ipc.postMessage('close');
}});
document.addEventListener('keydown', event => {{
  if (event.key === 'Escape' && window.ipc) window.ipc.postMessage('close');
}});
window.addEventListener('beforeunload', () => urls.forEach(URL.revokeObjectURL));
prepare();
</script>
</body>
</html>"#,
        chapters = chapters,
        transcript = transcript_html(TRANSCRIPT)
    )
}

fn transcript_html(markdown: &str) -> String {
    markdown
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with("# ") {
                None
            } else if let Some(heading) = line.strip_prefix("## ") {
                Some(format!("<h2>{}</h2>", html_escape(heading)))
            } else {
                Some(format!("<p>{}</p>", html_escape(line)))
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
#[path = "../tests/tutorial.rs"]
mod tests;
