//! Fan-out planning: turn a task into one run per agent.
//!
//! Given a task (identified by an id and a human title) and the set of agent
//! ids to run it against, produce a [`RunPlan`] per agent — each with a unique
//! branch name and worktree path so the agents never collide. The host then
//! creates the worktree (`git::worktree::create`) and the run row
//! (`store::Db::create_run`) from each plan.

/// A single agent's planned run of a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPlan {
    /// The agent id this run uses.
    pub agent: String,
    /// The branch to create for the worktree.
    pub branch: String,
    /// The worktree path, relative to the project root.
    pub worktree: String,
}

/// Plan a fan-out. `task_id` disambiguates branches across tasks with the same
/// title; `title` gives them a readable slug; `worktree_dir` is the configured
/// per-project worktree root (e.g. `.asylum/worktrees`). Agents are deduplicated
/// preserving order.
pub fn fanout(task_id: i64, title: &str, agents: &[String], worktree_dir: &str) -> Vec<RunPlan> {
    let slug = slugify(title);
    let base = if slug.is_empty() {
        format!("task-{task_id}")
    } else {
        format!("{slug}-{task_id}")
    };

    let mut seen = Vec::new();
    let mut plans = Vec::new();
    for agent in agents {
        if seen.contains(agent) {
            continue;
        }
        seen.push(agent.clone());
        let branch = format!("asylum/{base}-{agent}");
        let worktree = format!("{worktree_dir}/{base}-{agent}");
        plans.push(RunPlan {
            agent: agent.clone(),
            branch,
            worktree,
        });
    }
    plans
}

/// Lowercase, collapse non-alphanumerics to single hyphens, trim hyphens, and
/// cap length — for branch/worktree names derived from free text.
pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out.truncate(40);
    while out.ends_with('-') {
        out.pop();
    }
    out
}

#[cfg(test)]
#[path = "../tests/plan.rs"]
mod tests;
