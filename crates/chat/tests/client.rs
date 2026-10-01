use super::*;

fn chunks(parts: &[&str]) -> impl Stream<Item = reqwest::Result<Vec<u8>>> + Unpin {
  futures::stream::iter(parts.iter().map(|s| Ok(s.as_bytes().to_vec())).collect::<Vec<_>>())
}

#[tokio::test]
async fn decodes_text_calls_and_usage() {
  let s = chunks(&[
    "data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n",
    "data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n",
    "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c\",\"function\":{\"name\":\"shell\",\"arguments\":\"{}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n",
    "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5}}\n",
    "data: [DONE]\n",
  ]);
  let events: Vec<_> = decode(s).map(|e| e.unwrap()).collect().await;
  assert_eq!(events[0], StreamEvent::Text("Hel".into()));
  assert_eq!(events[1], StreamEvent::Text("lo".into()));
  assert!(matches!(events[2], StreamEvent::Usage(u) if u.total_tokens == 5));
  match &events[3] {
    StreamEvent::Done { calls, finish } => {
      assert_eq!(calls[0].function.name, "shell");
      assert_eq!(finish.as_deref(), Some("tool_calls"));
    }
    e => panic!("unexpected {e:?}"),
  }
}

#[tokio::test]
async fn ends_without_done_marker() {
  let s = chunks(&["data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\n"]);
  let events: Vec<_> = decode(s).map(|e| e.unwrap()).collect().await;
  assert_eq!(events.len(), 2);
  assert!(matches!(events[1], StreamEvent::Done { .. }));
}

#[test]
fn backoff_grows_and_caps() {
  let a = backoff(1).as_millis();
  assert!((375..=625).contains(&a));
  assert!(backoff(10).as_millis() <= 10_000);
  assert!(retryable(reqwest::StatusCode::TOO_MANY_REQUESTS));
  assert!(!retryable(reqwest::StatusCode::BAD_REQUEST));
}
