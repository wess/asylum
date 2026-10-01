use super::*;

#[test]
fn only_old_builds_behind_a_release() {
  let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
  assert!(required("0.2.0", "0.1.0", "2026-10-01", today));
  assert!(!required("0.2.0", "0.1.0", "2026-10-25", today));
  assert!(!required("0.1.0", "0.1.0", "2026-01-01", today));
  assert_eq!(age_days("bogus", today), 0);
}
