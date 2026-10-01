//! The background clock: fires due routines, expires approvals from work
//! that started without the user, runs the inactivity guard, and watches
//! the computer's disk.

use crate::event::Event;
use crate::queue::{Job, Origin};
use crate::runtime::Runtime;
use std::time::Duration;
use store::{approvals, routines, state};

pub const TICK: Duration = Duration::from_secs(15);
/// Away this long and the user is asked whether routines should keep going.
pub const AWAY_MS: i64 = 7 * 24 * 3600 * 1000;
/// No answer this long after asking pauses every routine.
pub const GRACE_MS: i64 = 24 * 3600 * 1000;
pub const LOW_DISK: u64 = 5 * 1024 * 1024 * 1024;
pub const CRITICAL_DISK: u64 = 1024 * 1024 * 1024;

pub fn spawn(rt: Runtime) {
  tokio::spawn(async move {
    let mut n: u64 = 0;
    loop {
      tick(&rt, n).await;
      n += 1;
      tokio::time::sleep(TICK).await;
    }
  });
}

async fn tick(rt: &Runtime, n: u64) {
  let now = store::now();
  if rt.settings().background_work {
    if let Ok(due) = routines::due(&rt.pool, now).await {
      for r in due {
        // Advance first so a slow run never double-fires.
        let next = schedule::next_run(&r.trigger, &r.schedule, now, rt.tz()).ok().flatten();
        let _ = routines::ran(&rt.pool, &r.id, now, next).await;
        if let Ok(chat) = store::chats::direct(&rt.pool, &r.bot_id).await {
          crate::turn::enqueue(rt, Job { bot: r.bot_id.clone(), chat: chat.id, origin: Origin::Routine, routine: Some(r.id.clone()), note: String::new() });
        }
      }
    }
  }
  if let Ok(stale) = approvals::expire_stale(&rt.pool, now).await {
    for a in stale {
      rt.emit(Event::Approval { id: a.id });
    }
  }
  inactivity(rt, now).await;
  crate::api::computer::wake_check(rt).await;
  crate::api::computer::hibernate(rt).await;
  if n.is_multiple_of(2) {
    let _ = crate::api::sync::tick(rt).await;
  }
  if n.is_multiple_of(4) {
    let _ = crate::policy::apply(rt, &config::policy::path()).await;
  }
  if n.is_multiple_of(20) {
    disk(rt).await;
  }
  crate::api::computer::terminate_inactive(rt, now).await;
  crate::api::computer::run_scheduled(rt, now).await;
  if n.is_multiple_of(240) {
    let _ = store::events::prune(&rt.pool, now).await;
    crate::api::computer::daily(rt).await;
  }
}

/// The UI records when the user was last present.
pub async fn seen(rt: &Runtime) {
  let _ = state::set(&rt.pool, "last_seen", &store::now().to_string()).await;
  let _ = state::remove(&rt.pool, "away_asked").await;
}

async fn inactivity(rt: &Runtime, now: i64) {
  let pool = &rt.pool;
  let last: i64 = state::get(pool, "last_seen").await.ok().flatten().and_then(|v| v.parse().ok()).unwrap_or(now);
  if now - last < AWAY_MS {
    return;
  }
  match state::get(pool, "away_asked").await.ok().flatten().and_then(|v| v.parse::<i64>().ok()) {
    None => {
      let _ = state::set(pool, "away_asked", &now.to_string()).await;
      rt.emit(Event::Notify {
        bot: None,
        chat: None,
        title: "Keep your routines running?".into(),
        body: "You've been away a while. Open the app to keep routines running, or they'll pause tomorrow.".into(),
      });
    }
    Some(asked) if now - asked > GRACE_MS => {
      if routines::pause_all(pool).await.unwrap_or(0) > 0 {
        rt.emit(Event::Notify { bot: None, chat: None, title: "Routines paused".into(), body: "They were paused while you were away.".into() });
      }
      let _ = state::set(pool, "away_asked", &i64::MAX.to_string()).await;
    }
    _ => {}
  }
}

async fn disk(rt: &Runtime) {
  let Some(free) = computer::disk_free(&rt.computer.root) else { return };
  let state = if free < CRITICAL_DISK {
    "critical"
  } else if free < LOW_DISK {
    "low"
  } else {
    "ok"
  };
  rt.emit(Event::Computer { state: format!("disk:{state}") });
}
