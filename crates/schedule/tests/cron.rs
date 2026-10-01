use super::*;

#[test]
fn parses_fields() {
  let c = Cron::parse("*/15 8-10 * * mon-fri").unwrap();
  assert_eq!(c.minutes, vec![0, 15, 30, 45]);
  assert_eq!(c.hours, vec![8, 9, 10]);
  assert_eq!(c.weekdays, vec![1, 2, 3, 4, 5]);
  assert!(c.any_day && !c.any_weekday);
}

#[test]
fn sunday_as_seven_and_names() {
  let c = Cron::parse("0 0 1 jan,jul 7").unwrap();
  assert_eq!(c.weekdays, vec![0]);
  assert_eq!(c.months, vec![1, 7]);
}

#[test]
fn rejects_bad() {
  assert!(Cron::parse("60 * * * *").is_err());
  assert!(Cron::parse("* * *").is_err());
  assert!(Cron::parse("*/0 * * * *").is_err());
  assert!(Cron::parse("5-1 * * * *").is_err());
}

#[test]
fn either_day_semantics() {
  let c = Cron::parse("0 0 13 * 5").unwrap();
  assert!(c.day_matches(13, 3, 2));
  assert!(c.day_matches(7, 3, 5));
  assert!(!c.day_matches(7, 3, 4));
}
