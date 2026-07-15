//! GitHub integration via the `gh` CLI.
//!
//! Rather than embed an HTTP client and OAuth, Asylum reuses the user's existing
//! `gh` authentication - the same tool their agents use. This crate shells out
//! to `gh ... --json ...` and parses the structured output into typed models:
//! browse pull requests and issues, and open a PR straight from a run's branch
//! (open a PR from the IDE). An issue can then seed a task/worktree.
//!
//! The parsing (`parse_prs`, `parse_issues`) is pure and tested against canned
//! JSON; the live calls are thin wrappers that require `gh` on `PATH`.

use std::path::Path;
use std::process::Command;

use serde::Deserialize;

/// A GitHub integration error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not run gh: {0}")]
    Spawn(String),
    #[error("gh: {0}")]
    Gh(String),
    #[error("malformed gh output: {0}")]
    Parse(String),
}

/// A pull request.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    #[serde(default, deserialize_with = "login")]
    pub author: String,
    pub state: String,
    #[serde(rename = "headRefName", default)]
    pub head: String,
    #[serde(rename = "baseRefName", default)]
    pub base: String,
    #[serde(rename = "isDraft", default)]
    pub draft: bool,
    #[serde(default)]
    pub url: String,
}

/// An issue.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    #[serde(default, deserialize_with = "login")]
    pub author: String,
    pub state: String,
    #[serde(default, deserialize_with = "label_names")]
    pub labels: Vec<String>,
    #[serde(default)]
    pub url: String,
}

/// gh nests author as `{ "login": "..." }`; flatten to the login string.
fn login<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    struct Author {
        #[serde(default)]
        login: String,
    }
    Ok(Option::<Author>::deserialize(d)?
        .map(|a| a.login)
        .unwrap_or_default())
}

/// gh nests labels as `[{ "name": "..." }]`; flatten to the names.
fn label_names<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    struct Label {
        #[serde(default)]
        name: String,
    }
    Ok(Vec::<Label>::deserialize(d)?
        .into_iter()
        .map(|l| l.name)
        .collect())
}

/// Parse `gh pr list --json ...` output.
pub fn parse_prs(json: &str) -> Result<Vec<PullRequest>, Error> {
    serde_json::from_str(json).map_err(|e| Error::Parse(e.to_string()))
}

/// Parse `gh issue list --json ...` output.
pub fn parse_issues(json: &str) -> Result<Vec<Issue>, Error> {
    serde_json::from_str(json).map_err(|e| Error::Parse(e.to_string()))
}

const PR_FIELDS: &str = "number,title,author,state,headRefName,baseRefName,isDraft,url";
const ISSUE_FIELDS: &str = "number,title,author,state,labels,url";

/// List open pull requests for the repo at `dir`.
pub fn pull_requests(dir: &Path, limit: u32) -> Result<Vec<PullRequest>, Error> {
    let out = gh(
        dir,
        &[
            "pr",
            "list",
            "--json",
            PR_FIELDS,
            "--limit",
            &limit.to_string(),
        ],
    )?;
    parse_prs(&out)
}

/// List open issues for the repo at `dir`.
pub fn issues(dir: &Path, limit: u32) -> Result<Vec<Issue>, Error> {
    let out = gh(
        dir,
        &[
            "issue",
            "list",
            "--json",
            ISSUE_FIELDS,
            "--limit",
            &limit.to_string(),
        ],
    )?;
    parse_issues(&out)
}

/// Open a pull request from `head` into `base`. Returns the new PR's URL.
pub fn create_pr(
    dir: &Path,
    title: &str,
    body: &str,
    base: &str,
    head: &str,
) -> Result<String, Error> {
    let out = gh(
        dir,
        &[
            "pr", "create", "--title", title, "--body", body, "--base", base, "--head", head,
        ],
    )?;
    // gh prints the PR URL as the last non-empty line.
    Ok(out
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string())
}

/// Slug for a branch/worktree derived from an issue: `issue-<n>-<title-slug>`.
pub fn issue_branch(issue: &Issue) -> String {
    let slug: String = slugify(&issue.title);
    if slug.is_empty() {
        format!("issue-{}", issue.number)
    } else {
        format!("issue-{}-{}", issue.number, slug)
    }
}

fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
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

fn gh(dir: &Path, args: &[&str]) -> Result<String, Error> {
    let out = Command::new("gh")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| Error::Spawn(e.to_string()))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(Error::Gh(if msg.is_empty() {
            "gh command failed".into()
        } else {
            msg
        }))
    }
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
