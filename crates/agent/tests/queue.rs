use super::*;

fn job(chat: &str, origin: Origin) -> Job {
  Job { bot: "b".into(), chat: chat.into(), origin, routine: None, note: String::new() }
}

#[test]
fn user_message_preempts_same_chat() {
  let q = Queues::default();
  assert!(matches!(q.push(job("c", Origin::Chat)), Push::Start));
  let (_, token) = q.next("b").unwrap();
  q.push(job("c", Origin::Routine));
  assert!(!token.is_cancelled());
  assert!(matches!(q.push(job("c", Origin::Chat)), Push::Queued));
  assert!(token.is_cancelled());
  // The chat job jumps ahead of the queued routine.
  assert_eq!(q.next("b").unwrap().0.origin, Origin::Chat);
  assert_eq!(q.next("b").unwrap().0.origin, Origin::Routine);
  assert!(q.next("b").is_none());
  assert!(!q.busy("b"));
}

#[test]
fn stop_clears() {
  let q = Queues::default();
  q.push(job("c", Origin::Chat));
  let (_, token) = q.next("b").unwrap();
  q.push(job("d", Origin::Handoff));
  q.stop("b");
  assert!(token.is_cancelled());
  assert!(q.next("b").is_none());
}
