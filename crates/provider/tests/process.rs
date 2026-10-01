use super::*;
use futures::StreamExt;

#[test]
fn substitutes_and_renders() {
  let a = args(&["--model".into(), "{model}".into(), "-C".into(), "{workspace}".into()], "opus", "/w");
  assert_eq!(a, vec!["--model", "opus", "-C", "/w"]);
  let r = render(&[Message::system("be brief"), Message::user("hi")]);
  assert!(r.starts_with("system:\nbe brief"));
  assert!(r.contains("user:\nhi"));
}

#[tokio::test(flavor = "multi_thread")]
async fn text_process_round_trip() {
  let dir = tempfile::tempdir().unwrap();
  let run = Run { command: "sh".into(), args: vec!["-c".into(), "tr a-z A-Z".into()], output: Output::Text, workspace: dir.path().into() };
  let mut s = run.stream("m", &[Message::user("hello")]).await.unwrap();
  let mut text = String::new();
  while let Some(ev) = s.next().await {
    if let StreamEvent::Text(t) = ev.unwrap() {
      text.push_str(&t);
    }
  }
  assert!(text.contains("USER:\nHELLO"));
}

#[tokio::test(flavor = "multi_thread")]
async fn stream_json_process() {
  let dir = tempfile::tempdir().unwrap();
  let script = r#"cat >/dev/null; printf '%s\n' '{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hi "}}}' '{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"there"}}}' '{"type":"result","result":"Hi there","usage":{"input_tokens":3,"output_tokens":2}}'"#;
  let run = Run { command: "sh".into(), args: vec!["-c".into(), script.into()], output: Output::StreamJson, workspace: dir.path().into() };
  let events: Vec<_> = run.stream("m", &[Message::user("x")]).await.unwrap().map(|e| e.unwrap()).collect().await;
  assert_eq!(events[0], StreamEvent::Text("Hi ".into()));
  assert_eq!(events[1], StreamEvent::Text("there".into()));
  assert!(matches!(events[2], StreamEvent::Usage(u) if u.total_tokens == 5));
  assert!(matches!(events[3], StreamEvent::Done { .. }));
}

#[tokio::test(flavor = "multi_thread")]
async fn failing_process_reports_stderr() {
  let dir = tempfile::tempdir().unwrap();
  let run = Run { command: "sh".into(), args: vec!["-c".into(), "echo nope >&2; exit 2".into()], output: Output::Text, workspace: dir.path().into() };
  let events: Vec<_> = run.stream("m", &[]).await.unwrap().collect().await;
  assert!(events[0].as_ref().unwrap_err().to_string().contains("nope"));
}
