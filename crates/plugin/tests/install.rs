use super::*;
use std::path::Path;

#[test]
fn parses_owner_repo() {
    let s = Source::parse("wess/herdr-reviewr").unwrap();
    assert_eq!(s.owner, "wess");
    assert_eq!(s.repo, "herdr-reviewr");
    assert_eq!(s.reference, None);
    assert_eq!(s.url(), "https://github.com/wess/herdr-reviewr.git");
    assert_eq!(s.dir_name(), "herdr-reviewr");
}

#[test]
fn parses_a_ref() {
    let s = Source::parse("acme/tool@v1.2.0").unwrap();
    assert_eq!(s.reference.as_deref(), Some("v1.2.0"));
}

#[test]
fn tolerates_urls_and_dot_git() {
    let a = Source::parse("https://github.com/acme/tool.git").unwrap();
    let b = Source::parse("git@github.com:acme/tool").unwrap();
    assert_eq!(a.owner, "acme");
    assert_eq!(a.repo, "tool");
    assert_eq!(b.repo, "tool");
}

#[test]
fn rejects_junk_and_traversal() {
    assert!(Source::parse("no-slash").is_err());
    assert!(Source::parse("/repo").is_err());
    assert!(Source::parse("owner/").is_err());
    assert!(Source::parse("../etc/passwd").is_err());
    assert!(Source::parse("owner/../evil").is_err());
    assert!(Source::parse("own er/repo").is_err());
}

#[test]
fn clone_command_is_shallow_and_targets_the_repo_dir() {
    let s = Source::parse("acme/tool").unwrap();
    let (program, argv) = clone_command(&s, Path::new("/plugins"));
    assert_eq!(program, "git");
    assert_eq!(argv[0], "clone");
    assert!(argv.contains(&"--depth".to_string()));
    assert!(argv.contains(&"https://github.com/acme/tool.git".to_string()));
    assert_eq!(argv.last().unwrap(), "/plugins/tool");
    assert!(!argv.contains(&"--branch".to_string()));
}

#[test]
fn clone_command_passes_a_ref_as_branch() {
    let s = Source::parse("acme/tool@main").unwrap();
    let (_, argv) = clone_command(&s, Path::new("/plugins"));
    let i = argv.iter().position(|a| a == "--branch").unwrap();
    assert_eq!(argv[i + 1], "main");
}

#[test]
fn discover_command_filters_by_topic() {
    let (program, argv) = discover_command(20);
    assert_eq!(program, "gh");
    assert!(argv.contains(&format!("--topic={TOPIC}")));
    assert!(argv.contains(&"--limit=20".to_string()));
    assert_eq!(TOPIC, "asylum-plugin");
}
