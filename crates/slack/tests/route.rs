use super::*;

fn ev(kind: &str, ct: &str, text: &str, thread: Option<&str>) -> Event {
  Event { kind: kind.into(), channel: "C1".into(), channel_type: ct.into(), user: "U9".into(), text: text.into(), ts: "100.1".into(), thread_ts: thread.map(Into::into), ..Default::default() }
}

#[test]
fn dms_always_answered() {
  assert_eq!(decide(&ev("message", "im", "hi", None), "UBOT", &HashSet::new()), Decision::Answer { conversation: "im:C1".into(), thread: None, follow: false });
}

#[test]
fn channel_needs_mention_then_follows_thread() {
  let none = HashSet::new();
  assert_eq!(decide(&ev("message", "channel", "hello all", None), "UBOT", &none), Decision::Ignore);
  let d = decide(&ev("app_mention", "channel", "<@UBOT> help", None), "UBOT", &none);
  assert_eq!(d, Decision::Answer { conversation: "C1:100.1".into(), thread: Some("100.1".into()), follow: true });
  let followed: HashSet<String> = ["C1:100.1".to_string()].into();
  let reply = ev("message", "channel", "and also", Some("100.1"));
  assert!(matches!(decide(&reply, "UBOT", &followed), Decision::Answer { follow: false, .. }));
  assert_eq!(decide(&ev("message", "channel", "other thread", Some("200.2")), "UBOT", &followed), Decision::Ignore);
}

#[test]
fn ignores_self_and_bots() {
  let mut e = ev("message", "im", "x", None);
  e.bot_id = Some("B1".into());
  assert_eq!(decide(&e, "UBOT", &HashSet::new()), Decision::Ignore);
  let mut e = ev("message", "im", "x", None);
  e.user = "UBOT".into();
  assert_eq!(decide(&e, "UBOT", &HashSet::new()), Decision::Ignore);
  assert_eq!(clean("<@UBOT> do it", "UBOT"), "do it");
}
