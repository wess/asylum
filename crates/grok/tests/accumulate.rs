use super::*;
use crate::types::FunctionDelta;

fn delta(index: usize, id: Option<&str>, name: Option<&str>, args: Option<&str>) -> CallDelta {
  CallDelta {
    index,
    id: id.map(Into::into),
    function: Some(FunctionDelta {
      name: name.map(Into::into),
      arguments: args.map(Into::into),
    }),
  }
}

#[test]
fn stitches_fragments() {
  let mut a = Accumulator::default();
  a.push(&delta(0, Some("c1"), Some("shell"), Some("{\"cmd\":")));
  a.push(&delta(1, Some("c2"), Some("read"), None));
  a.push(&delta(0, None, None, Some("\"ls\"}")));
  let calls = a.finish();
  assert_eq!(calls.len(), 2);
  assert_eq!(calls[0].function.arguments, "{\"cmd\":\"ls\"}");
  assert_eq!(calls[1].function.arguments, "{}");
}

#[test]
fn fills_missing_ids() {
  let mut a = Accumulator::default();
  a.push(&delta(0, None, Some("x"), Some("{}")));
  assert_eq!(a.finish()[0].id, "call_0");
}
