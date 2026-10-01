use super::*;

#[test]
fn spec_round_trip() {
  let s = Spec { shape: "tall".into(), eyes: "wink".into(), accessory: "halo".into(), tone: "#ff6b6b".into() };
  let f = format(&s);
  assert_eq!(parse(f.trim_start_matches("bot:")), s);
  assert_eq!(parse("junk"), Spec::default());
}
