use super::*;
use store::{Bot, Chat, Memory, Skill};

fn ctx() -> Context {
  let bot = Bot { id: "me".into(), name: "Scout".into(), label: "Talent Scout".into(), description: "Never email candidates without approval.".into(), kind: "personal".into(), ..Default::default() };
  let other = Bot { id: "o".into(), name: "Writer".into(), label: "Copy".into(), kind: "personal".into(), ..Default::default() };
  Context {
    chat: Chat { kind: "direct".into(), ..Default::default() },
    members: vec![],
    roster: vec![bot.clone(), other],
    memory: vec![Memory { kind: "preference".into(), scope: "bot".into(), content: "Prefers bullet summaries".into(), ..Default::default() }],
    skills: vec![Skill { name: "Weekly pipeline".into(), description: "pull the CRM list".into(), ..Default::default() }],
    routines: vec![],
    secrets: vec!["CRM_TOKEN".into()],
    history: vec![],
    plugins: vec![],
    user: "Wess".into(),
    now: "Monday".into(),
    zone: "UTC".into(),
    workspace: "/w".into(),
    language: "system".into(),
    model: "grok-4".into(),
    bot,
  }
}

#[test]
fn includes_identity_rules_memory_skills_roster() {
  let s = system(&ctx());
  assert!(s.contains("You are Scout, an AI teammate whose job is: Talent Scout"));
  assert!(s.contains("Never email candidates without approval."));
  assert!(s.contains("[preference] Prefers bullet summaries"));
  assert!(s.contains("/Weekly pipeline: pull the CRM list"));
  assert!(s.contains("- Writer (Copy)"));
  assert!(s.contains("CRM_TOKEN"));
  assert!(!s.contains("group chat"));
  assert!(s.contains("running on the model grok-4"));
}

#[test]
fn group_prompt_names_members_and_description() {
  let mut c = ctx();
  c.chat = Chat { kind: "group".into(), title: "Launch".into(), description: "Ship v2 Friday".into(), ..Default::default() };
  c.members = vec![c.roster[1].clone()];
  let s = system(&c);
  assert!(s.contains("This is a group chat"));
  assert!(s.contains("Ship v2 Friday"));
}
