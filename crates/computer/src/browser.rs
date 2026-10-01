//! The computer's browser: one Chromium over CDP whose profile every Bot
//! shares (so a sign-in done once works for all), with a page per Bot — its
//! screen. Bots see a page as text plus a numbered list of interactive
//! elements and act by number; the user sees screenshots and, on takeover,
//! drives the page with real mouse and keyboard events.

use crate::clip::clip;
use anyhow::{anyhow, bail, Context, Result};
use chromiumoxide::browser::{Browser as Chrome, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::input::{
  DispatchKeyEventParams, DispatchKeyEventType, DispatchMouseEventParams,
  DispatchMouseEventType, InsertTextParams, MouseButton,
};
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::Page;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

pub const WIDTH: u32 = 1280;
pub const HEIGHT: u32 = 800;

struct Live {
  chrome: Chrome,
  pages: HashMap<String, Page>,
  _handler: tokio::task::JoinHandle<()>,
}

#[derive(Clone)]
pub struct Browser {
  profile: PathBuf,
  live: Arc<Mutex<Option<Live>>>,
  /// Show a real window (for passkeys, security keys, and other things
  /// only a visible browser can do); headless otherwise.
  headful: Arc<std::sync::atomic::AtomicBool>,
  /// When a screen was last used (Unix seconds), for idle hibernation.
  used: Arc<std::sync::atomic::AtomicU64>,
  /// Egress proxy port for Network Controls (0: direct).
  proxy: Arc<std::sync::atomic::AtomicU16>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
  pub url: String,
  pub title: String,
  pub text: String,
  pub elements: Vec<String>,
}

impl Snapshot {
  pub fn render(&self) -> String {
    let mut s = format!("URL: {}\nTitle: {}\n\n", self.url, self.title);
    s.push_str(&clip(&self.text, 12_000));
    if !self.elements.is_empty() {
      s.push_str("\n\nInteractive elements (use the number):\n");
      s.push_str(&self.elements.join("\n"));
    }
    s
  }
}

/// One user interaction captured while recording a Teach-a-task demo.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Step {
  pub kind: String,
  pub target: String,
  #[serde(default)]
  pub value: String,
  pub url: String,
  pub at: f64,
}

const MARK_JS: &str = r#"(() => {
  const sel = 'a[href],button,input,textarea,select,[role=button],[role=link],[role=tab],[role=menuitem],[role=checkbox],[contenteditable=true],summary,label[for]';
  document.querySelectorAll('[data-asylum]').forEach(e => e.removeAttribute('data-asylum'));
  const out = [];
  let n = 0;
  for (const el of document.querySelectorAll(sel)) {
    const r = el.getBoundingClientRect();
    const st = getComputedStyle(el);
    if (r.width < 2 || r.height < 2 || st.visibility === 'hidden' || st.display === 'none') continue;
    if (r.bottom < 0 || r.top > innerHeight * 3) continue;
    n += 1;
    el.setAttribute('data-asylum', String(n));
    const tag = el.tagName.toLowerCase();
    const type = el.getAttribute('type') || '';
    const label = (el.getAttribute('aria-label') || el.innerText || el.value || el.getAttribute('placeholder') || el.getAttribute('title') || el.getAttribute('name') || '').trim().replace(/\s+/g, ' ').slice(0, 80);
    const href = tag === 'a' ? ' -> ' + (el.getAttribute('href') || '').slice(0, 80) : '';
    const secret = type === 'password' ? ' (password)' : '';
    out.push('[' + n + '] ' + tag + (type ? ':' + type : '') + ' "' + label + '"' + secret + href);
    if (n >= 150) break;
  }
  return { url: location.href, title: document.title, text: (document.body ? document.body.innerText : '').slice(0, 40000), elements: out };
})()"#;

const RECORD_JS: &str = r#"(() => {
  if (window.__asylumRec) return true;
  window.__asylumRec = [];
  const describe = el => {
    if (!el || !el.tagName) return '';
    const label = (el.getAttribute('aria-label') || el.innerText || el.getAttribute('placeholder') || el.getAttribute('name') || el.id || '').trim().replace(/\s+/g, ' ').slice(0, 80);
    return el.tagName.toLowerCase() + ' "' + label + '"';
  };
  const push = (kind, el, value) => window.__asylumRec.push({ kind, target: describe(el), value: value || '', url: location.href, at: Date.now() });
  addEventListener('click', e => push('click', e.target), true);
  addEventListener('change', e => {
    const t = e.target;
    const secret = t && t.type === 'password';
    push('input', t, secret ? '[secret]' : (t.value || '').slice(0, 200));
  }, true);
  addEventListener('submit', e => push('submit', e.target), true);
  addEventListener('keydown', e => { if (e.key === 'Enter') push('key', e.target, 'Enter'); }, true);
  push('page', document.body, document.title);
  return true;
})()"#;

impl Browser {
  pub fn new(profile: PathBuf) -> Self {
    Self {
      profile,
      live: Arc::new(Mutex::new(None)),
      headful: Arc::new(std::sync::atomic::AtomicBool::new(false)),
      used: Arc::new(std::sync::atomic::AtomicU64::new(now_secs())),
      proxy: Arc::new(std::sync::atomic::AtomicU16::new(0)),
    }
  }

  /// Route the browser through the egress proxy (or not). Takes effect on
  /// the next launch; returns whether it changed.
  pub fn set_proxy(&self, port: Option<u16>) -> bool {
    self.proxy.swap(port.unwrap_or(0), std::sync::atomic::Ordering::SeqCst) != port.unwrap_or(0)
  }

  /// Seconds since a Bot or a viewer last touched a screen.
  pub fn idle_secs(&self) -> u64 {
    now_secs().saturating_sub(self.used.load(std::sync::atomic::Ordering::SeqCst))
  }

  pub fn is_headful(&self) -> bool {
    self.headful.load(std::sync::atomic::Ordering::SeqCst)
  }

  /// Relaunch visibly (or back to headless), reopening each Bot's page.
  /// Sign-ins survive because the profile is the same.
  pub async fn set_headful(&self, on: bool) -> Result<()> {
    if self.is_headful() == on {
      return Ok(());
    }
    let mut open: Vec<(String, String)> = Vec::new();
    {
      let guard = self.live.lock().await;
      if let Some(live) = guard.as_ref() {
        for (bot, page) in &live.pages {
          if let Ok(Some(url)) = page.url().await {
            open.push((bot.clone(), url));
          }
        }
      }
    }
    self.shutdown().await;
    self.headful.store(on, std::sync::atomic::Ordering::SeqCst);
    for (bot, url) in open {
      let _ = self.open(&bot, &url).await;
    }
    Ok(())
  }

  pub async fn running(&self) -> bool {
    self.live.lock().await.is_some()
  }

  async fn launch(&self) -> Result<Live> {
    std::fs::create_dir_all(&self.profile)?;
    reclaim(&self.profile);
    let headful = self.headful.load(std::sync::atomic::Ordering::SeqCst);
    let mut builder = BrowserConfig::builder();
    builder = if headful { builder.with_head() } else { builder.new_headless_mode() };
    let mut builder = builder
      .user_data_dir(&self.profile)
      .window_size(WIDTH, HEIGHT)
      .viewport(None)
      .arg("--hide-scrollbars")
      .arg("--no-first-run")
      .arg("--no-default-browser-check");
    let proxy = self.proxy.load(std::sync::atomic::Ordering::SeqCst);
    if proxy != 0 {
      builder = builder.arg(format!("--proxy-server=http://127.0.0.1:{proxy}"));
    }
    if let Some(exe) = find_chrome() {
      builder = builder.chrome_executable(exe);
    }
    let config = builder.build().map_err(|e| anyhow!(e))?;
    let (chrome, mut handler) = Chrome::launch(config)
      .await
      .context("could not start the browser (is Chrome, Edge, or Chromium installed?)")?;
    let task = tokio::spawn(async move { while handler.next().await.is_some() {} });
    Ok(Live {
      chrome,
      pages: HashMap::new(),
      _handler: task,
    })
  }

  async fn page(&self, bot: &str) -> Result<Page> {
    self.used.store(now_secs(), std::sync::atomic::Ordering::SeqCst);
    let mut guard = self.live.lock().await;
    if guard.is_none() {
      *guard = Some(self.launch().await?);
    }
    let live = guard.as_mut().expect("launched");
    if let Some(p) = live.pages.get(bot) {
      if p.url().await.is_ok() {
        return Ok(p.clone());
      }
    }
    let page = live.chrome.new_page("about:blank").await?;
    page
      .execute(
        chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams::new(
          WIDTH as i64,
          HEIGHT as i64,
          1.0,
          false,
        ),
      )
      .await?;
    live.pages.insert(bot.to_string(), page.clone());
    Ok(page)
  }

  /// The Bot's current URL, or `None` if it has no screen yet.
  pub async fn url(&self, bot: &str) -> Option<String> {
    let guard = self.live.lock().await;
    let page = guard.as_ref()?.pages.get(bot)?.clone();
    drop(guard);
    page.url().await.ok().flatten()
  }

  pub async fn open(&self, bot: &str, url: &str) -> Result<Snapshot> {
    let page = self.page(bot).await?;
    page.goto(crate::web::normalize(url)).await?;
    settle(&page).await;
    snapshot(&page).await
  }

  pub async fn snapshot(&self, bot: &str) -> Result<Snapshot> {
    snapshot(&self.page(bot).await?).await
  }

  pub async fn click(&self, bot: &str, index: u32) -> Result<Snapshot> {
    let page = self.page(bot).await?;
    let (x, y) = center(&page, index).await?;
    mouse_click(&page, x, y).await?;
    settle(&page).await;
    snapshot(&page).await
  }

  pub async fn type_text(&self, bot: &str, index: u32, text: &str, submit: bool) -> Result<Snapshot> {
    let page = self.page(bot).await?;
    focus_clear(&page, index).await?;
    page.execute(InsertTextParams::new(text)).await?;
    if submit {
      key(&page, "Enter").await?;
    }
    settle(&page).await;
    snapshot(&page).await
  }

  /// Fill a field with a value the model never sees (a stored secret).
  pub async fn fill_secret(&self, bot: &str, index: u32, value: &str) -> Result<()> {
    let page = self.page(bot).await?;
    focus_clear(&page, index).await?;
    page.execute(InsertTextParams::new(value)).await?;
    Ok(())
  }

  pub async fn press(&self, bot: &str, name: &str) -> Result<Snapshot> {
    let page = self.page(bot).await?;
    key(&page, name).await?;
    settle(&page).await;
    snapshot(&page).await
  }

  pub async fn scroll(&self, bot: &str, dy: i64) -> Result<Snapshot> {
    let page = self.page(bot).await?;
    page.evaluate(format!("window.scrollBy(0, {dy})")).await?;
    snapshot(&page).await
  }

  pub async fn back(&self, bot: &str) -> Result<Snapshot> {
    let page = self.page(bot).await?;
    page.evaluate("history.back()").await?;
    settle(&page).await;
    snapshot(&page).await
  }

  pub async fn forward(&self, bot: &str) -> Result<Snapshot> {
    let page = self.page(bot).await?;
    page.evaluate("history.forward()").await?;
    settle(&page).await;
    snapshot(&page).await
  }

  pub async fn reload(&self, bot: &str) -> Result<()> {
    self.page(bot).await?.reload().await?;
    Ok(())
  }

  /// A PNG of the Bot's screen.
  pub async fn screenshot(&self, bot: &str) -> Result<Vec<u8>> {
    let page = self.page(bot).await?;
    let params = ScreenshotParams::builder()
      .format(CaptureScreenshotFormat::Png)
      .build();
    Ok(page.screenshot(params).await?)
  }

  /// Takeover input: a click at page coordinates.
  pub async fn pointer(&self, bot: &str, x: f64, y: f64) -> Result<()> {
    mouse_click(&self.page(bot).await?, x, y).await
  }

  pub async fn wheel(&self, bot: &str, x: f64, y: f64, dy: f64) -> Result<()> {
    let page = self.page(bot).await?;
    let p = DispatchMouseEventParams::builder()
      .r#type(DispatchMouseEventType::MouseWheel)
      .x(x)
      .y(y)
      .delta_x(0.0)
      .delta_y(dy)
      .build()
      .map_err(|e| anyhow!(e))?;
    page.execute(p).await?;
    Ok(())
  }

  /// Takeover input: typed text or a named key.
  pub async fn keystroke(&self, bot: &str, text: Option<&str>, named: Option<&str>) -> Result<()> {
    let page = self.page(bot).await?;
    if let Some(t) = text.filter(|t| !t.is_empty()) {
      page.execute(InsertTextParams::new(t)).await?;
    }
    if let Some(k) = named {
      key(&page, k).await?;
    }
    Ok(())
  }

  pub async fn start_recording(&self, bot: &str) -> Result<()> {
    let page = self.page(bot).await?;
    page.evaluate_on_new_document(RECORD_JS).await?;
    page.evaluate(RECORD_JS).await?;
    Ok(())
  }

  /// Collect the demo's steps and stop listening for new pages.
  pub async fn stop_recording(&self, bot: &str) -> Result<Vec<Step>> {
    let page = self.page(bot).await?;
    let steps: Vec<Step> = page
      .evaluate("(() => { const r = window.__asylumRec || []; window.__asylumRec = null; return r; })()")
      .await?
      .into_value()
      .unwrap_or_default();
    Ok(steps)
  }

  pub async fn close_screen(&self, bot: &str) {
    let mut guard = self.live.lock().await;
    if let Some(live) = guard.as_mut() {
      if let Some(p) = live.pages.remove(bot) {
        let _ = p.close().await;
      }
    }
  }

  pub async fn shutdown(&self) {
    let mut guard = self.live.lock().await;
    if let Some(mut live) = guard.take() {
      let _ = live.chrome.close().await;
      let _ = live.chrome.wait().await;
    }
  }

  /// Sign out of every site: drop all cookies in the shared profile.
  pub async fn clear_cookies(&self) -> Result<()> {
    let guard = self.live.lock().await;
    if let Some(live) = guard.as_ref() {
      live.chrome.clear_cookies().await?;
    }
    Ok(())
  }
}

/// A previous run that died without cleaning up (killed, crashed) can leave
/// its Chromium alive and holding the profile. The profile's SingletonLock
/// names that process as `host-pid`; stop it if — and only if — its command
/// line is using this profile, then clear the stale locks.
pub fn reclaim(profile: &std::path::Path) {
  let lock = profile.join("SingletonLock");
  if let Ok(target) = std::fs::read_link(&lock) {
    let pid = target.to_string_lossy().rsplit('-').next().and_then(|p| p.parse::<i32>().ok());
    if let Some(pid) = pid {
      let cmd = std::process::Command::new("ps").args(["-o", "command=", "-p", &pid.to_string()]).output();
      let ours = cmd
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&*profile.to_string_lossy()))
        .unwrap_or(false);
      if ours {
        let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
        std::thread::sleep(std::time::Duration::from_millis(400));
      }
    }
  }
  for name in ["SingletonLock", "SingletonSocket", "SingletonCookie"] {
    let _ = std::fs::remove_file(profile.join(name));
  }
}

pub fn find_chrome() -> Option<PathBuf> {
  [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ]
  .iter()
  .map(PathBuf::from)
  .find(|p| p.exists())
}

async fn settle(page: &Page) {
  let _ = tokio::time::timeout(std::time::Duration::from_secs(8), page.wait_for_navigation()).await;
  tokio::time::sleep(std::time::Duration::from_millis(350)).await;
}

async fn snapshot(page: &Page) -> Result<Snapshot> {
  let snap: Snapshot = page.evaluate(MARK_JS).await?.into_value()?;
  Ok(snap)
}

async fn center(page: &Page, index: u32) -> Result<(f64, f64)> {
  let js = format!(
    "(() => {{ const el = document.querySelector('[data-asylum=\"{index}\"]'); if (!el) return null; el.scrollIntoView({{block:'center'}}); const r = el.getBoundingClientRect(); return [r.left + r.width/2, r.top + r.height/2]; }})()"
  );
  let v: Option<(f64, f64)> = page.evaluate(js).await?.into_value().unwrap_or(None);
  v.ok_or_else(|| anyhow!("no element [{index}] on the page; take a fresh look first"))
}

async fn focus_clear(page: &Page, index: u32) -> Result<()> {
  let js = format!(
    "(() => {{ const el = document.querySelector('[data-asylum=\"{index}\"]'); if (!el) return false; el.scrollIntoView({{block:'center'}}); el.focus(); if ('value' in el) {{ el.value = ''; el.dispatchEvent(new Event('input', {{bubbles:true}})); }} else if (el.isContentEditable) {{ el.textContent = ''; }} return true; }})()"
  );
  let ok: bool = page.evaluate(js).await?.into_value().unwrap_or(false);
  if !ok {
    bail!("no element [{index}] on the page; take a fresh look first");
  }
  Ok(())
}

async fn mouse_click(page: &Page, x: f64, y: f64) -> Result<()> {
  for kind in [
    DispatchMouseEventType::MouseMoved,
    DispatchMouseEventType::MousePressed,
    DispatchMouseEventType::MouseReleased,
  ] {
    let p = DispatchMouseEventParams::builder()
      .r#type(kind)
      .x(x)
      .y(y)
      .button(MouseButton::Left)
      .click_count(1)
      .build()
      .map_err(|e| anyhow!(e))?;
    page.execute(p).await?;
  }
  Ok(())
}

/// Named keys the model and takeover can send, with their virtual key codes.
pub fn key_code(name: &str) -> Option<(&'static str, i64)> {
  Some(match name.to_lowercase().as_str() {
    "enter" | "return" => ("Enter", 13),
    "tab" => ("Tab", 9),
    "escape" | "esc" => ("Escape", 27),
    "backspace" => ("Backspace", 8),
    "delete" => ("Delete", 46),
    "arrowup" | "up" => ("ArrowUp", 38),
    "arrowdown" | "down" => ("ArrowDown", 40),
    "arrowleft" | "left" => ("ArrowLeft", 37),
    "arrowright" | "right" => ("ArrowRight", 39),
    "pageup" => ("PageUp", 33),
    "pagedown" => ("PageDown", 34),
    "home" => ("Home", 36),
    "end" => ("End", 35),
    "space" => (" ", 32),
    _ => return None,
  })
}

async fn key(page: &Page, name: &str) -> Result<()> {
  let (k, code) = key_code(name).ok_or_else(|| anyhow!("unknown key {name}"))?;
  let text = match k {
    "Enter" => Some("\r"),
    " " => Some(" "),
    _ => None,
  };
  for kind in [DispatchKeyEventType::KeyDown, DispatchKeyEventType::KeyUp] {
    let mut b = DispatchKeyEventParams::builder()
      .r#type(kind.clone())
      .key(k)
      .code(if k == " " { "Space" } else { k })
      .windows_virtual_key_code(code)
      .native_virtual_key_code(code);
    if kind == DispatchKeyEventType::KeyDown {
      if let Some(t) = text {
        b = b.text(t);
      }
    }
    page.execute(b.build().map_err(|e| anyhow!(e))?).await?;
  }
  Ok(())
}

#[cfg(test)]
#[path = "../tests/browser.rs"]
mod tests;

fn now_secs() -> u64 {
  std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
