use super::*;

#[test]
fn common_schedules() {
  assert_eq!(describe("schedule", "0 8 * * 1-5", "{}"), "Every weekday at 8:00 AM");
  assert_eq!(describe("schedule", "30 17 * * *", "{}"), "Every day at 5:30 PM");
  assert_eq!(describe("schedule", "0 */2 * * *", "{}"), "Every day every 2 hours");
  assert_eq!(describe("schedule", "0 9 1 * *", "{}"), "On the 1st of the month at 9:00 AM");
  assert_eq!(describe("schedule", "0 9,17 * * mon", "{}"), "Every Monday at 9:00 AM and 5:00 PM");
  assert_eq!(describe("interval", "120", "{}"), "Every 2 hours");
  assert_eq!(describe("interval", "1", "{}"), "Every minute");
  assert_eq!(describe("slack", "", "{\"channel\":\"#eng\"}"), "When a Slack event happens (channel: #eng)");
}
