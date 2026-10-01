use super::*;

#[test]
fn presets_match_ainz() {
  let l = build("lite-llm", "litellm");
  assert_eq!(l.endpoint, "http://127.0.0.1:4000/v1");
  assert_eq!(l.credential, Credential::Env { var: "LITELLM_API_KEY".into() });
  assert_eq!(build("ollama", "o").credential, Credential::None);
  assert_eq!(build("ollama-cloud", "oc").endpoint, "https://api.ollama.com/v1");
  let c = build("claude-code", "claude");
  assert_eq!(c.kind, Kind::Process);
  assert_eq!(c.output, Output::StreamJson);
  assert!(c.args.contains(&"{model}".to_string()));
  assert!(!c.models.is_empty());
  assert_eq!(build("anthropic", "claude").endpoint, "https://api.anthropic.com/v1");
  assert_eq!(build("custom", "gw").credential, Credential::Keychain { account: "provider-gw".into() });
}
