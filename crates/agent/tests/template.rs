use super::*;

#[test]
fn link_round_trip_and_warnings() {
  let t = Template {
    name: "Scout".into(),
    description: "Use key sk-live-123 at http://crm.internal".into(),
    skills: vec![SkillDef { name: "s".into(), ..Default::default() }],
    ..Default::default()
  };
  let l = link(&t).unwrap();
  assert!(l.starts_with(SCHEME));
  assert_eq!(parse(&l).unwrap(), t);
  let w = warnings(&t);
  assert_eq!(w.len(), 2);
  assert!(parse("https://x").is_err());
}
