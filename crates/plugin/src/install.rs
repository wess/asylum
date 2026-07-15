//! Install plugins from GitHub, and discover community plugins by topic.
//!
//! Plugins are just directories with a `plugin.toml`, so "installing" one is a
//! shallow `git clone` into the plugins directory - `asylum plugin install
//! owner/repo`. Discovery leans on a shared GitHub topic ([`TOPIC`]): any repo
//! tagged with it is a candidate, found via the `gh` CLI.
//!
//! The parsing and command-building here are pure and unit-tested; the actual
//! clone ([`fetch`]) shells out to `git`, mirroring how the `github` crate wraps
//! `gh`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The GitHub topic community plugins tag themselves with, so `asylum plugin
/// search` can find them.
pub const TOPIC: &str = "asylum-plugin";

/// A parsed `owner/repo[@ref]` install spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub owner: String,
    pub repo: String,
    /// An optional branch, tag, or commit to check out.
    pub reference: Option<String>,
}

impl Source {
    /// Parse `owner/repo` or `owner/repo@ref`. Rejects empty segments, path
    /// traversal, and characters that have no business in a repo slug.
    pub fn parse(spec: &str) -> Result<Source, String> {
        let spec = spec.trim();
        // Tolerate a pasted URL.
        let spec = spec
            .strip_prefix("https://github.com/")
            .or_else(|| spec.strip_prefix("git@github.com:"))
            .unwrap_or(spec);
        let (path, reference) = match spec.split_once('@') {
            Some((p, r)) if !r.is_empty() => (p, Some(r.to_string())),
            _ => (spec, None),
        };
        let path = path.strip_suffix(".git").unwrap_or(path);
        let (owner, repo) = path
            .split_once('/')
            .ok_or_else(|| format!("expected owner/repo, got `{spec}`"))?;
        for (label, part) in [("owner", owner), ("repo", repo)] {
            if part.is_empty() {
                return Err(format!("{label} is empty in `{spec}`"));
            }
            if part.contains("..") || !part.chars().all(is_slug_char) {
                return Err(format!("invalid {label} `{part}`"));
            }
        }
        if let Some(r) = &reference {
            if r.contains("..")
                || r.contains('/') && !r.chars().all(|c| is_slug_char(c) || c == '/')
            {
                return Err(format!("invalid ref `{r}`"));
            }
        }
        Ok(Source {
            owner: owner.to_string(),
            repo: repo.to_string(),
            reference,
        })
    }

    /// The HTTPS clone URL.
    pub fn url(&self) -> String {
        format!("https://github.com/{}/{}.git", self.owner, self.repo)
    }

    /// The directory name the plugin installs into (the repo name).
    pub fn dir_name(&self) -> String {
        self.repo.clone()
    }
}

fn is_slug_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}

/// Build the `(program, argv)` for a shallow clone of `source` into
/// `dir/<repo>`. A `reference` adds `--branch <ref>`.
pub fn clone_command(source: &Source, dir: &Path) -> (String, Vec<String>) {
    let dest = dir.join(source.dir_name());
    let mut argv = vec!["clone".to_string(), "--depth".to_string(), "1".to_string()];
    if let Some(reference) = &source.reference {
        argv.push("--branch".to_string());
        argv.push(reference.clone());
    }
    argv.push(source.url());
    argv.push(dest.to_string_lossy().into_owned());
    ("git".to_string(), argv)
}

/// Build the `gh` command that lists repos carrying the [`TOPIC`] topic. `limit`
/// caps results. The output is JSON (`nameWithOwner`, `description`, `url`).
pub fn discover_command(limit: u32) -> (String, Vec<String>) {
    (
        "gh".to_string(),
        vec![
            "search".to_string(),
            "repos".to_string(),
            format!("--topic={TOPIC}"),
            format!("--limit={limit}"),
            "--json".to_string(),
            "nameWithOwner,description,url,stargazersCount".to_string(),
        ],
    )
}

/// Clone `spec` into `dir`, returning the installed plugin directory. Fails if
/// the destination already exists, the clone fails, or the result has no
/// `plugin.toml`. Shells out - not exercised by unit tests.
pub fn fetch(spec: &str, dir: &Path) -> Result<PathBuf, String> {
    let source = Source::parse(spec)?;
    let dest = dir.join(source.dir_name());
    if dest.exists() {
        return Err(format!(
            "{} is already installed (remove {} to reinstall)",
            source.dir_name(),
            dest.display()
        ));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let (program, argv) = clone_command(&source, dir);
    let status = Command::new(&program)
        .args(&argv)
        .status()
        .map_err(|e| format!("could not run {program}: {e}"))?;
    if !status.success() {
        return Err(format!("clone of {} failed", source.url()));
    }
    if !dest.join(crate::MANIFEST).exists() {
        let _ = std::fs::remove_dir_all(&dest);
        return Err(format!(
            "{} has no {} - is it an Asylum plugin?",
            source.dir_name(),
            crate::MANIFEST
        ));
    }
    Ok(dest)
}

#[cfg(test)]
#[path = "../tests/install.rs"]
mod tests;
