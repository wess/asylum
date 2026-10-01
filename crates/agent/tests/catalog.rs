use super::*;

#[test]
fn skills_name_real_connectors() {
  for s in SKILLS {
    for id in s.needs {
      assert!(plugin(id).is_some(), "{} needs unknown connector {id}", s.name);
    }
  }
}

#[test]
fn jira_is_reachable_two_ways() {
  let hosted = plugin("jira").unwrap();
  assert_eq!(hosted.kind, "oauth");
  assert_eq!(hosted.url, "https://mcp.atlassian.com/v1/mcp");
  let token = plugin("jiratoken").unwrap();
  assert_eq!((token.kind, token.command, token.args), ("command", "uvx", &["mcp-atlassian"][..]));
  let required: Vec<&str> = token.fields.iter().filter(|f| !f.optional).map(|f| f.name).collect();
  assert_eq!(required, ["JIRA_URL", "JIRA_USERNAME", "JIRA_API_TOKEN"]);
  assert!(token.fields.iter().find(|f| f.name == "JIRA_API_TOKEN").unwrap().secret);
  let jira: Vec<&str> = SKILLS.iter().filter(|s| s.needs == JIRA).map(|s| s.name).collect();
  assert_eq!(jira.len(), 7);
  assert!(SKILLS.iter().filter(|s| s.needs == JIRA).all(|s| s.instructions.contains("jira_search") && s.instructions.contains("Ask") || s.instructions.contains("ask")));
}

#[test]
fn jira_reads_run_and_writes_ask() {
  use crate::approve::{plugin_class, Class};
  for read in ["jira_search", "jira_get_issue", "jira_get_transitions", "searchJiraIssuesUsingJql", "getJiraIssue", "lookupJiraAccountId", "getAccessibleAtlassianResources"] {
    assert_eq!(plugin_class(read, false), Class::Free, "{read}");
  }
  for write in ["jira_create_issue", "jira_update_issue", "jira_transition_issue", "jira_add_comment", "jira_delete_issue", "createJiraIssue", "transitionJiraIssue", "addCommentToJiraIssue"] {
    assert_eq!(plugin_class(write, false), Class::Consequential, "{write}");
  }
}
