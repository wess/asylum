use super::*;

#[test]
fn assistant_with_calls_omits_empty_content() {
  let m = Message::assistant("", vec![ToolCall::new("a", "shell", "{}")]);
  let v = serde_json::to_value(&m).unwrap();
  assert!(v.get("content").is_none());
  assert_eq!(v["tool_calls"][0]["function"]["name"], "shell");
}

#[test]
fn tool_message_carries_call_id() {
  let v = serde_json::to_value(Message::tool("x1", "ok")).unwrap();
  assert_eq!(v["role"], "tool");
  assert_eq!(v["tool_call_id"], "x1");
}

#[test]
fn parts_serialize_openai_shape() {
  let m = Message::user_parts(vec![Part::text("hi"), Part::image("data:image/png;base64,AA")]);
  let v = serde_json::to_value(&m).unwrap();
  assert_eq!(v["content"][1]["type"], "image_url");
  assert_eq!(v["content"][1]["image_url"]["url"], "data:image/png;base64,AA");
}

#[test]
fn bad_arguments_read_as_empty_object() {
  let c = ToolCall::new("a", "b", "{not json");
  assert!(c.args().as_object().unwrap().is_empty());
}
