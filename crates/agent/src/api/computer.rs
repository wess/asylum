//! The computer's lifecycle: status, daily backups, update, recover, and
//! reset — with the guards the product documents: no second operation while
//! one runs ("Update still running"), no update or reset without a backup
//! ("Backup not ready"), and none while a Bot can't pause ("Agent busy").

use crate::event::Event;
use crate::runtime::Runtime;
use anyhow::{bail, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

static BUSY: AtomicBool = AtomicBool::new(false);
pub const KEEP: usize = 7;

pub const STARTING: &str = "starting";
pub const READY: &str = "ready";
pub const UPDATING: &str = "updating";
pub const RECOVERING: &str = "recovering";
pub const RESETTING: &str = "resetting";
pub const UNREACHABLE: &str = "unreachable";
pub const HIBERNATING: &str = "hibernating";
pub const RECREATING: &str = "recreating";

static ASLEEP: AtomicBool = AtomicBool::new(false);

/// How long recreate waits for Bots to reach a safe point.
pub const SAFE_POINT: std::time::Duration = std::time::Duration::from_secs(120);

struct Guard;

impl Guard {
  fn take() -> Result<Self> {
    if BUSY.swap(true, Ordering::SeqCst) {
      bail!("Update still running");
    }
    Ok(Guard)
  }
}

impl Drop for Guard {
  fn drop(&mut self) {
    BUSY.store(false, Ordering::SeqCst);
  }
}

fn emit(rt: &Runtime, state: &str) {
  rt.emit(Event::Computer { state: state.into() });
  if [UPDATING, RECOVERING, RESETTING, RECREATING].contains(&state) {
    let (rt, name) = (rt.clone(), format!("computer.{state}"));
    tokio::spawn(async move { crate::audit::change(&rt, "user", &name, "", "").await });
  }
}

/// Bring the computer up: directories, then a browser probe.
pub async fn start(rt: &Runtime) -> Result<()> {
  emit(rt, STARTING);
  rt.computer.ensure()?;
  match rt.browser.snapshot("_probe").await {
    Ok(_) => {
      rt.browser.close_screen("_probe").await;
      emit(rt, READY);
      let rt = rt.clone();
      tokio::spawn(async move { setup(&rt).await });
      Ok(())
    }
    Err(e) => {
      emit(rt, UNREACHABLE);
      Err(e)
    }
  }
}

pub fn snapshots(rt: &Runtime) -> Vec<PathBuf> {
  let mut v: Vec<PathBuf> = std::fs::read_dir(rt.computer.snapshots())
    .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "tgz")).collect())
    .unwrap_or_default();
  v.sort();
  v
}

/// Back up the workspace (skipping replaceable dependency folders). Keeps
/// the most recent week.
pub async fn backup(rt: &Runtime) -> Result<PathBuf> {
  let ws = rt.computer.workspace();
  let dir = rt.computer.snapshots();
  std::fs::create_dir_all(&dir)?;
  let name = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
  let path = dir.join(format!("{name}.tgz"));
  let status = tokio::process::Command::new("tar")
    .args(["--exclude", "node_modules", "--exclude", ".venv", "--exclude", "venv", "--exclude", "target", "-czf"])
    .arg(&path)
    .arg("-C")
    .arg(&ws)
    .arg(".")
    .status()
    .await?;
  if !status.success() {
    bail!("backup failed");
  }
  let all = snapshots(rt);
  if all.len() > KEEP {
    for old in &all[..all.len() - KEEP] {
      let _ = std::fs::remove_file(old);
    }
  }
  Ok(path)
}

/// Back up once a day.
pub async fn daily(rt: &Runtime) {
  let today = chrono::Utc::now().format("%Y%m%d").to_string();
  let has = snapshots(rt).iter().any(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&today)));
  if !has {
    let _ = backup(rt).await;
  }
}

fn preflight(rt: &Runtime) -> Result<()> {
  if rt.queues.busy_any() {
    bail!("Agent busy: a Bot is working and can't pause yet. Try again when it finishes.");
  }
  if snapshots(rt).is_empty() {
    bail!("Backup not ready: the first backup hasn't finished yet.");
  }
  Ok(())
}

/// A software update keeps everything: restart the browser on a fresh
/// profile lock. A computer update also clears replaceable state (caches,
/// temp files, dependency folders) but keeps files and sign-ins.
pub async fn update(rt: &Runtime, full: bool) -> Result<()> {
  let _g = Guard::take()?;
  preflight(rt)?;
  emit(rt, UPDATING);
  backup(rt).await?;
  rt.browser.shutdown().await;
  if full {
    let ws = rt.computer.workspace();
    for junk in [".cache", "tmp", ".tmp"] {
      let _ = std::fs::remove_dir_all(ws.join(junk));
    }
    let _ = std::fs::remove_dir_all(rt.computer.profile().join("Default/Cache"));
    let _ = std::fs::remove_dir_all(rt.computer.profile().join("Default/Code Cache"));
  }
  start(rt).await
}

/// From the unreachable state: save what's possible, rebuild the pieces
/// that can break (browser locks and caches), and bring it back.
pub async fn recover(rt: &Runtime) -> Result<()> {
  let _g = Guard::take()?;
  emit(rt, RECOVERING);
  let _ = backup(rt).await;
  rt.browser.shutdown().await;
  let _ = std::fs::remove_dir_all(rt.computer.profile().join("Default/Cache"));
  let _ = std::fs::remove_dir_all(rt.computer.profile().join("ShaderCache"));
  start(rt).await
}

/// Last resort: restore the workspace from the latest backup.
pub async fn reset(rt: &Runtime) -> Result<()> {
  let _g = Guard::take()?;
  if rt.queues.busy_any() {
    bail!("Agent busy: a Bot is working and can't pause yet.");
  }
  let Some(last) = snapshots(rt).pop() else { bail!("Backup not ready: there is no backup to reset from.") };
  emit(rt, RESETTING);
  rt.browser.shutdown().await;
  let ws = rt.computer.workspace();
  let _ = std::fs::remove_dir_all(&ws);
  std::fs::create_dir_all(&ws)?;
  let status = tokio::process::Command::new("tar").arg("-xzf").arg(&last).arg("-C").arg(&ws).status().await?;
  if !status.success() {
    bail!("Retry Reset: the backup could not be restored.");
  }
  start(rt).await
}

/// Sign out of every site in the shared browser.
pub async fn sign_out_sites(rt: &Runtime) -> Result<()> {
  rt.browser.clear_cookies().await
}

#[derive(Clone, Debug, Default)]
pub struct Disk {
  pub workspace: u64,
  pub browser: u64,
  pub backups: u64,
  pub free: Option<u64>,
}

pub fn disk(rt: &Runtime) -> Disk {
  Disk {
    workspace: computer::disk_usage(&rt.computer.workspace()),
    browser: computer::disk_usage(&rt.computer.profile()),
    backups: computer::disk_usage(&rt.computer.snapshots()),
    free: computer::disk_free(&rt.computer.root),
  }
}

/// Idle hibernation: with no Bot working and no screen touched for the
/// configured time, stop the browser to free memory. It wakes on its own
/// the next time a Bot (or a viewer) uses a screen.
pub async fn hibernate(rt: &Runtime) -> bool {
  let minutes = rt.settings().hibernate_minutes;
  minutes > 0 && hibernate_after(rt, u64::from(minutes) * 60).await
}

pub async fn hibernate_after(rt: &Runtime, idle: u64) -> bool {
  if ASLEEP.load(Ordering::SeqCst) || BUSY.load(Ordering::SeqCst) || rt.queues.busy_any() || rt.browser.is_headful() {
    return false;
  }
  if rt.browser.idle_secs() < idle || !rt.browser.running().await {
    return false;
  }
  rt.browser.shutdown().await;
  ASLEEP.store(true, Ordering::SeqCst);
  emit(rt, HIBERNATING);
  true
}

/// Report waking once something has used the computer again.
pub async fn wake_check(rt: &Runtime) {
  if ASLEEP.load(Ordering::SeqCst) && rt.browser.running().await {
    ASLEEP.store(false, Ordering::SeqCst);
    emit(rt, READY);
  }
}

pub fn asleep() -> bool {
  ASLEEP.load(Ordering::SeqCst)
}

/// Wake now (opening the computer panel, or Resume).
pub async fn wake(rt: &Runtime) -> Result<()> {
  ASLEEP.store(false, Ordering::SeqCst);
  start(rt).await
}

/// A fresh computer on the same durable disk: Bots pause at a safe point
/// (between jobs), the browser and everything replaceable is rebuilt —
/// files, sign-ins, and backups stay — and paused work resumes.
pub async fn recreate(rt: &Runtime) -> Result<()> {
  let _g = Guard::take()?;
  let Ok(_pause) = tokio::time::timeout(SAFE_POINT, rt.gate.write()).await else {
    bail!("Agent busy: a Bot is working and can't pause yet.");
  };
  emit(rt, RECREATING);
  backup(rt).await?;
  rt.browser.shutdown().await;
  let profile = rt.computer.profile();
  for junk in ["Default/Cache", "Default/Code Cache", "ShaderCache", "GrShaderCache", "Crashpad", "SingletonLock", "SingletonSocket", "SingletonCookie"] {
    let p = profile.join(junk);
    let _ = std::fs::remove_dir_all(&p);
    let _ = std::fs::remove_file(&p);
  }
  let ws = rt.computer.workspace();
  for junk in [".cache", "tmp", ".tmp"] {
    let _ = std::fs::remove_dir_all(ws.join(junk));
  }
  ASLEEP.store(false, Ordering::SeqCst);
  start(rt).await
}

/// Stop the computer now (it starts again when a Bot needs it).
pub async fn stop(rt: &Runtime) -> Result<()> {
  if rt.queues.busy_any() {
    bail!("Agent busy: a Bot is working and can't pause yet.");
  }
  rt.browser.shutdown().await;
  ASLEEP.store(true, Ordering::SeqCst);
  emit(rt, HIBERNATING);
  Ok(())
}

/// Delete the computer and its data: workspace files, browser sign-ins,
/// and backups. Bots and conversations stay; the computer starts empty.
pub async fn delete_all(rt: &Runtime) -> Result<()> {
  let _g = Guard::take()?;
  let Ok(_pause) = tokio::time::timeout(SAFE_POINT, rt.gate.write()).await else {
    bail!("Agent busy: a Bot is working and can't pause yet.");
  };
  emit(rt, RESETTING);
  rt.browser.shutdown().await;
  for dir in [rt.computer.workspace(), rt.computer.profile(), rt.computer.snapshots()] {
    let _ = std::fs::remove_dir_all(&dir);
  }
  rt.computer.ensure()?;
  ASLEEP.store(false, Ordering::SeqCst);
  start(rt).await
}

/// The admin's setup script (once per script) and check script (every
/// start), run on the computer. A failure is reported, not fatal.
pub async fn setup(rt: &Runtime) {
  let p = rt.policy();
  let digest = format!("{:x}", md5_like(&p.setup_script));
  let done = store::state::get(&rt.pool, "setup_done").await.ok().flatten().unwrap_or_default();
  if !p.setup_script.trim().is_empty() && done != digest {
    match script(rt, &p.setup_script).await {
      Ok(_) => {
        let _ = store::state::set(&rt.pool, "setup_done", &digest).await;
        crate::audit::change(rt, "admin", "computer.setup", "", "ok").await;
      }
      Err(e) => report(rt, "Setup script failed", &e.to_string()).await,
    }
  }
  if !p.check_script.trim().is_empty() {
    if let Err(e) = script(rt, &p.check_script).await {
      report(rt, "Check script failed", &e.to_string()).await;
    }
  }
}

async fn script(rt: &Runtime, body: &str) -> Result<String> {
  let ws = rt.computer.workspace();
  let out = computer::shell::run(computer::shell::Spec {
    command: body,
    cwd: &ws,
    place: computer::shell::Place::Computer,
    env: &[],
    timeout: std::time::Duration::from_secs(600),
    net: rt.net(),
  })
  .await?;
  if out.code != Some(0) {
    bail!("{}", out.render());
  }
  Ok(out.render())
}

async fn report(rt: &Runtime, title: &str, body: &str) {
  let _ = store::notifications::add(&rt.pool, None, None, "error", title, body).await;
  crate::audit::change(rt, "admin", "computer.script_failed", title, body).await;
  rt.emit(Event::Notify { bot: None, chat: None, title: title.into(), body: body.chars().take(300).collect() });
}

/// A stable fingerprint of a script (not cryptographic; detects edits).
fn md5_like(s: &str) -> u64 {
  s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3))
}

/// Terminate a computer unused for the admin's limit: delete its data.
pub async fn terminate_inactive(rt: &Runtime, now: i64) {
  if rt.queues.busy_any() || rt.browser.idle_secs() < 60 {
    let _ = store::state::set(&rt.pool, "computer_used", &now.to_string()).await;
    return;
  }
  let days = rt.policy().terminate_inactive_days;
  if days == 0 {
    return;
  }
  let last: i64 = store::state::get(&rt.pool, "computer_used").await.ok().flatten().and_then(|v| v.parse().ok()).unwrap_or(now);
  if now - last > i64::from(days) * 24 * 3600 * 1000 && delete_all(rt).await.is_ok() {
    let _ = store::state::set(&rt.pool, "computer_used", &now.to_string()).await;
    crate::audit::change(rt, "admin", "computer.terminated_inactive", &days.to_string(), "").await;
  }
}

/// Schedule a computer update for the next 2:00 AM local time (when Bots
/// are least likely to be busy), or cancel it.
pub async fn schedule_update(rt: &Runtime, on: bool) -> Result<Option<i64>> {
  if !on {
    store::state::set(&rt.pool, "update_at", "").await?;
    return Ok(None);
  }
  let at = next_quiet_hour(rt, store::now());
  store::state::set(&rt.pool, "update_at", &at.to_string()).await?;
  crate::audit::change(rt, "user", "computer.update_scheduled", &at.to_string(), "").await;
  Ok(Some(at))
}

pub async fn scheduled_update(rt: &Runtime) -> Option<i64> {
  store::state::get(&rt.pool, "update_at").await.ok().flatten().and_then(|v| v.parse().ok())
}

/// The next 2:00 in the user's timezone after `now` (ms).
pub fn next_quiet_hour(rt: &Runtime, now: i64) -> i64 {
  let next = schedule::next_run("schedule", "0 2 * * *", now, rt.tz()).ok().flatten();
  next.unwrap_or(now + 24 * 3600 * 1000)
}

/// Run a scheduled update once its time comes (and no Bot is busy).
pub async fn run_scheduled(rt: &Runtime, now: i64) {
  let Some(at) = scheduled_update(rt).await else { return };
  if now < at || rt.queues.busy_any() {
    return;
  }
  let _ = store::state::set(&rt.pool, "update_at", "").await;
  let _ = update(rt, true).await;
}
