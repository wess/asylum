use super::*;

fn bot(id: &str, name: &str) -> Bot {
  Bot { id: id.into(), name: name.into(), ..Default::default() }
}

fn msg(role: &str, bot: Option<&str>, body: &str, parts: &str) -> Message {
  Message { role: role.into(), bot_id: bot.map(Into::into), body: body.into(), parts: parts.into(), ..Default::default() }
}

#[test]
fn replays_tools_and_names_other_bots() {
  let me = bot("a", "Scout");
  let roster = vec![me.clone(), bot("b", "Writer")];
  let tool = r#"[{"type":"tool","id":"c1","name":"shell","args":{"command":"ls"},"result":"x.txt","status":"done"}]"#;
  let hist = vec![
    msg("user", None, "list files", "[]"),
    msg("bot", Some("a"), "Found x.txt", tool),
    msg("bot", Some("b"), "I can write it up", "[]"),
    msg("user", None, "thanks", "[]"),
  ];
  let out = build(&me, &hist, &roster, &|_| None);
  assert_eq!(out[0].role, grok::Role::User);
  assert_eq!(out[1].tool_calls.as_ref().unwrap()[0].function.name, "shell");
  assert_eq!(out[2].role, grok::Role::Tool);
  assert_eq!(out[3].content.as_ref().unwrap().as_text(), "Found x.txt");
  // The other Bot's line and the user's follow-up merge into one user turn.
  let last = out.last().unwrap().content.as_ref().unwrap().as_text();
  assert!(last.starts_with("[Writer]: I can write it up"));
  assert!(last.ends_with("thanks"));
}

#[test]
fn images_attach_as_parts() {
  let me = bot("a", "Scout");
  let att = r#"[{"type":"attachment","name":"p.png","path":"/w/p.png","mime":"image/png","size":3}]"#;
  let out = build(&me, &[msg("user", None, "see", att)], std::slice::from_ref(&me), &|_| Some("data:image/png;base64,AA".into()));
  match out[0].content.as_ref().unwrap() {
    grok::Content::Parts(p) => assert_eq!(p.len(), 2),
    _ => panic!("expected parts"),
  }
}
