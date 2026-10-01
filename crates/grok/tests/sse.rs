use super::*;

#[test]
fn split_across_chunks() {
  let mut d = Decoder::default();
  assert!(d.push(b"data: {\"a\"").is_empty());
  assert_eq!(d.push(b":1}\n\n"), vec![Frame::Data("{\"a\":1}".into())]);
}

#[test]
fn done_and_comments() {
  let mut d = Decoder::default();
  let frames = d.push(b": keepalive\r\ndata: x\r\ndata: [DONE]\n");
  assert_eq!(frames, vec![Frame::Data("x".into()), Frame::Done]);
}
