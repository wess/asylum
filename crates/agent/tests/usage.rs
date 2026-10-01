use super::*;

#[test]
fn weeks_start_monday() {
  let d = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(); // Thursday
  assert_eq!(week_start(d), NaiveDate::from_ymd_opt(2026, 9, 28).unwrap());
  assert_eq!(month_start(d), NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());
}

#[test]
fn blocking_rules() {
  let mut s = Summary { week: 10, weekly_limit: 10, month_over: 0, monthly_limit: 5, on_demand: false };
  assert!(s.blocked());
  s.on_demand = true;
  assert!(!s.blocked());
  s.month_over = 5;
  assert!(s.blocked());
  s.week = 9;
  assert!(!s.blocked());
}
