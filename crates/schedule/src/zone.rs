use chrono_tz::Tz;

/// The system timezone ("Auto-detect"), falling back to UTC.
pub fn local_zone() -> Tz {
  iana_time_zone::get_timezone()
    .ok()
    .and_then(|name| name.parse().ok())
    .unwrap_or(Tz::UTC)
}

/// A zone by IANA name; empty or "auto" means the system zone.
pub fn zone(name: &str) -> Tz {
  match name.trim() {
    "" | "auto" => local_zone(),
    n => n.parse().unwrap_or_else(|_| local_zone()),
  }
}
