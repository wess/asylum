use super::*;

#[test]
fn parses_pr_list() {
    let json = r#"[
        {"number":42,"title":"Add login","author":{"login":"alice"},"state":"OPEN",
         "headRefName":"feature","baseRefName":"main","isDraft":false,"url":"https://x/42"},
        {"number":43,"title":"WIP","author":{"login":"bob"},"state":"OPEN",
         "headRefName":"wip","baseRefName":"main","isDraft":true,"url":"https://x/43"}
    ]"#;
    let prs = parse_prs(json).unwrap();
    assert_eq!(prs.len(), 2);
    assert_eq!(prs[0].number, 42);
    assert_eq!(prs[0].author, "alice");
    assert_eq!(prs[0].head, "feature");
    assert!(!prs[0].draft);
    assert!(prs[1].draft);
}

#[test]
fn parses_issue_list_with_labels() {
    let json = r#"[
        {"number":7,"title":"Bug: crash","author":{"login":"carol"},"state":"OPEN",
         "labels":[{"name":"bug"},{"name":"p1"}],"url":"https://x/i/7"}
    ]"#;
    let issues = parse_issues(json).unwrap();
    assert_eq!(issues[0].number, 7);
    assert_eq!(issues[0].labels, vec!["bug", "p1"]);
    assert_eq!(issues[0].author, "carol");
}

#[test]
fn issue_branch_naming() {
    let issue = Issue {
        number: 12,
        title: "Fix the Flaky Test!".into(),
        author: "x".into(),
        state: "OPEN".into(),
        labels: vec![],
        url: String::new(),
    };
    assert_eq!(issue_branch(&issue), "issue-12-fix-the-flaky-test");

    let untitled = Issue {
        number: 5,
        title: "!!!".into(),
        author: String::new(),
        state: "OPEN".into(),
        labels: vec![],
        url: String::new(),
    };
    assert_eq!(issue_branch(&untitled), "issue-5");
}

#[test]
fn malformed_json_is_error() {
    assert!(matches!(parse_prs("not json"), Err(Error::Parse(_))));
}

#[test]
fn empty_list() {
    assert!(parse_prs("[]").unwrap().is_empty());
    assert!(parse_issues("[]").unwrap().is_empty());
}
