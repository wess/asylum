//! Playing audio files (voice memos) with the system player.

use std::collections::HashMap;
use std::process::Child;
use std::sync::Mutex;

static PLAYING: Mutex<Option<HashMap<String, Child>>> = Mutex::new(None);

pub fn playing(path: &str) -> bool {
  let mut g = PLAYING.lock().expect("audio");
  let map = g.get_or_insert_with(HashMap::new);
  match map.get_mut(path) {
    Some(c) => matches!(c.try_wait(), Ok(None)),
    None => false,
  }
}

pub fn toggle(path: &str) {
  let mut g = PLAYING.lock().expect("audio");
  let map = g.get_or_insert_with(HashMap::new);
  if let Some(mut c) = map.remove(path) {
    if matches!(c.try_wait(), Ok(None)) {
      let _ = c.kill();
      return;
    }
  }
  for (_, mut c) in map.drain() {
    let _ = c.kill();
  }
  let player = if cfg!(target_os = "macos") { "afplay" } else { "paplay" };
  if let Ok(c) = std::process::Command::new(player).arg(path).spawn() {
    map.insert(path.to_string(), c);
  }
}
