use super::*;
use chrono::TimeZone;

fn ms(tz: Tz, y: i32, mo: u32, d: u32, h: u32, mi: u32) -> i64 {
  tz.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap().with_timezone(&Utc).timestamp_millis()
}

#[test]
fn weekday_morning_in_zone() {
  let tz: Tz = "America/New_York".parse().unwrap();
  // Friday 2026-10-02 09:00 -> next weekday 8:00 is Monday 2026-10-05.
  let after = ms(tz, 2026, 10, 2, 9, 0);
  let next = next_cron("0 8 * * 1-5", after, tz).unwrap();
  assert_eq!(next, ms(tz, 2026, 10, 5, 8, 0));
}

#[test]
fn strictly_after() {
  let tz = Tz::UTC;
  let at = ms(tz, 2026, 1, 1, 8, 0);
  assert_eq!(next_cron("0 8 * * *", at, tz).unwrap(), ms(tz, 2026, 1, 2, 8, 0));
}

#[test]
fn every_two_hours_and_interval() {
  let tz = Tz::UTC;
  let at = ms(tz, 2026, 1, 1, 8, 30);
  assert_eq!(next_cron("0 */2 * * *", at, tz).unwrap(), ms(tz, 2026, 1, 1, 10, 0));
  assert_eq!(next_run("interval", "90", at, tz).unwrap(), Some(at + 90 * 60_000));
  assert_eq!(next_run("webhook", "", at, tz).unwrap(), None);
}

#[test]
fn leap_day() {
  let tz = Tz::UTC;
  let at = ms(tz, 2026, 3, 1, 0, 0);
  assert_eq!(next_cron("0 0 29 2 *", at, tz).unwrap(), ms(tz, 2028, 2, 29, 0, 0));
}
