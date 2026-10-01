use super::*;

#[test]
fn plugin_reads_are_free_writes_are_not() {
  assert_eq!(plugin_class("search_issues", false), Class::Free);
  assert_eq!(plugin_class("list_channels", false), Class::Free);
  assert_eq!(plugin_class("ask_wiki_question", false), Class::Free);
  assert_eq!(plugin_class("create_issue", false), Class::Consequential);
  assert_eq!(plugin_class("send_message", false), Class::Consequential);
  assert_eq!(plugin_class("anything", true), Class::Free);
}

#[test]
fn signatures() {
  assert_eq!(signature("shell", ""), "Always allow `shell`");
  assert_eq!(signature("github__create_issue", "wess/asylum"), "Always allow `github__create_issue` on `wess/asylum`");
}
