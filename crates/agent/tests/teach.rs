use super::*;

#[test]
fn renders_steps_with_offsets() {
  let steps = vec![
    Step { kind: "page".into(), target: "body \"\"".into(), value: "CRM".into(), url: "https://crm".into(), at: 1000.0 },
    Step { kind: "input".into(), target: "input \"Password\"".into(), value: "[secret]".into(), url: "https://crm".into(), at: 4000.0 },
  ];
  let r = render(&steps);
  assert!(r.contains("1. [0s] page"));
  assert!(r.contains("2. [3s] input input \"Password\" = \"[secret]\""));
}

#[test]
fn parses_draft() {
  let (d, i) = parse("```{\"description\":\"d\",\"instructions\":\"1. x\"}```").unwrap();
  assert_eq!((d.as_str(), i.as_str()), ("d", "1. x"));
}
