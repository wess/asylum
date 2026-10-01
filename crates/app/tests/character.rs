use super::*;

#[test]
fn spec_parses_and_falls_back() {
  let s = Spec { shape: "tall".into(), eyes: "wink".into(), accessory: "halo".into(), tone: "#ff6b6b".into() };
  assert_eq!(parse("shape=tall;eyes=wink;accessory=halo;tone=#ff6b6b"), s);
  assert_eq!(parse("junk"), Spec::default());
}
