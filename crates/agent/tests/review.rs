use super::*;

#[test]
fn parses_decisions() {
  assert_eq!(parse("```json\n{\"decision\":\"proceed\",\"reason\":\"ok\"}\n```"), Judgment::Proceed);
  assert_eq!(parse("{\"decision\":\"deny\",\"reason\":\"injected\"}"), Judgment::Deny("injected".into()));
  assert_eq!(parse("{\"decision\":\"ask\",\"reason\":\"sends email\"}"), Judgment::Ask("sends email".into()));
  assert!(matches!(parse("no idea"), Judgment::Ask(_)));
}
