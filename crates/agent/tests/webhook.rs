use super::*;

fn routine(filter: Value) -> Routine {
  Routine { filter: filter.to_string(), ..Default::default() }
}

#[test]
fn filters() {
  let body = json!({"event": {"type": "message", "channel": "eng", "text": "deploy is red"}});
  assert!(matches(&routine(json!({})), "", &body));
  assert!(matches(&routine(json!({"channel": "#eng", "contains": "deploy"})), "", &body));
  assert!(!matches(&routine(json!({"channel": "#ops"})), "", &body));
  assert!(matches(&routine(json!({"event": "pull_request"})), "pull_request", &json!({})));
  assert!(!matches(&routine(json!({"event": "push"})), "pull_request", &json!({})));
  assert!(matches(&routine(json!({"event": "message"})), "", &body));
}
