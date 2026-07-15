use super::*;

#[test]
fn slugify_basics() {
    assert_eq!(slugify("Add Login Feature!"), "add-login-feature");
    assert_eq!(slugify("  weird__name  "), "weird-name");
    assert_eq!(slugify("---"), "");
    assert_eq!(slugify(""), "");
}

#[test]
fn slugify_caps_length() {
    let long = "a".repeat(100);
    assert!(slugify(&long).len() <= 40);
}

#[test]
fn fanout_one_plan_per_agent() {
    let agents = vec!["claude-code".to_string(), "codex".to_string()];
    let plans = fanout(7, "Add login", &agents, ".asylum/worktrees");
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0].agent, "claude-code");
    assert_eq!(plans[0].branch, "asylum/add-login-7-claude-code");
    assert_eq!(
        plans[0].worktree,
        ".asylum/worktrees/add-login-7-claude-code"
    );
    assert_eq!(plans[1].branch, "asylum/add-login-7-codex");
}

#[test]
fn fanout_dedups_agents() {
    let agents = vec!["codex".to_string(), "codex".to_string()];
    let plans = fanout(1, "T", &agents, "wt");
    assert_eq!(plans.len(), 1);
}

#[test]
fn fanout_untitled_uses_task_id() {
    let plans = fanout(42, "!!!", &["codex".to_string()], "wt");
    assert_eq!(plans[0].branch, "asylum/task-42-codex");
}
