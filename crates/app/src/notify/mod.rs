//! Desktop notifications and the dock badge.
//!
//! macOS notifications go through UserNotifications so alerts carry the
//! app's own icon; a dev build without a bundle identity falls back to
//! `osascript` (which shows Script Editor's icon).

#[cfg(target_os = "macos")]
mod mac;

/// Post a notification without blocking the caller.
pub fn post(title: &str, body: &str) {
  let (title, body) = (title.to_string(), body.to_string());
  std::thread::spawn(move || send(&title, &body));
}

pub fn send(title: &str, body: &str) {
  #[cfg(target_os = "macos")]
  {
    if mac::send(title, body) {
      return;
    }
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!("display notification \"{}\" with title \"{}\"", esc(body), esc(title));
    let _ = std::process::Command::new("osascript").args(["-e", &script]).output();
  }
  #[cfg(target_os = "linux")]
  {
    let _ = std::process::Command::new("notify-send").args(["--app-name=Asylum", title, body]).output();
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux")))]
  let _ = (title, body);
}

/// Show `count` on the dock icon (nothing at zero). Main thread only.
pub fn badge(count: usize) {
  #[cfg(target_os = "macos")]
  {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::NSString;
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    let tile = app.dockTile();
    let label = if count == 0 { None } else { Some(NSString::from_str(&count.min(999).to_string())) };
    tile.setBadgeLabel(label.as_deref());
  }
  #[cfg(not(target_os = "macos"))]
  let _ = count;
}
