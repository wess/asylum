//! Update checks against GitHub Releases.
//!
//! Asylum ships as a packaged desktop app; this crate answers "is a newer
//! release available?" without embedding an HTTP client. It shells out to
//! `curl` for the public `releases/latest` endpoint (no auth needed) and parses
//! the result. Version comparison and release parsing are pure and tested; the
//! single network call is a thin wrapper.
//!
//! The app checks on startup and, when a newer release exists, notifies the user
//! with the download URL. Actually swapping the binary is left to the platform
//! package (Homebrew, the `.deb`, or a re-download), which the notification
//! links to.

use std::process::Command;

use serde::Deserialize;

/// An update-check error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not run curl: {0}")]
    Spawn(String),
    #[error("release request failed: {0}")]
    Request(String),
    #[error("malformed release output: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// A semantic version, compared field by field. Pre-release/build suffixes are
/// ignored so `0.2.0-rc.1` compares equal to `0.2.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    /// Parse `v1.2.3`, `1.2.3`, `1.2`, or `1` (missing fields default to 0).
    /// A pre-release or build suffix (`-rc.1`, `+build`) is dropped.
    pub fn parse(text: &str) -> Option<Version> {
        let core = text
            .trim()
            .trim_start_matches(['v', 'V'])
            .split(['-', '+'])
            .next()
            .unwrap_or_default();
        if core.is_empty() {
            return None;
        }
        let mut parts = core.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        Some(Version {
            major,
            minor,
            patch,
        })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A published release worth offering the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    /// The release title, e.g. "Widgets galore" (distinct from the `tag`).
    pub name: String,
    pub version: Version,
    /// The release page on the web (GitHub's `html_url`).
    pub url: String,
    /// ISO 8601, as GitHub reports it (e.g. `2026-01-02T03:04:05Z`).
    pub published_at: String,
    /// Release notes body, truncated to [`NOTES_CAP`] with an ellipsis marker.
    pub notes: String,
}

/// The result of a check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Running the latest (or newer) version.
    UpToDate,
    /// A newer release is available.
    Available(Release),
    /// The latest version could not be determined (kept non-fatal).
    Unknown,
}

/// The subset of the GitHub release payload we need.
#[derive(Debug, Deserialize)]
struct ReleasePayload {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    published_at: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// Cap on cached release notes: a long changelog is truncated rather than
/// held (and later shown) in full.
const NOTES_CAP: usize = 4096;

/// Truncate `notes` to at most [`NOTES_CAP`] bytes, on a char boundary, and
/// mark the cut so a shortened changelog doesn't read as complete.
fn truncate_notes(notes: &str) -> String {
    if notes.len() <= NOTES_CAP {
        return notes.to_string();
    }
    let mut end = NOTES_CAP;
    while end > 0 && !notes.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…\n\n(truncated)", &notes[..end])
}

/// Compare the running version against a GitHub `releases/latest` JSON payload.
/// Pure, so it is tested against canned JSON.
pub fn evaluate(current: &str, latest_json: &str) -> Status {
    let Some(current) = Version::parse(current) else {
        return Status::Unknown;
    };
    let Ok(payload) = serde_json::from_str::<ReleasePayload>(latest_json) else {
        return Status::Unknown;
    };
    if payload.draft || payload.prerelease {
        return Status::UpToDate;
    }
    let Some(version) = Version::parse(&payload.tag_name) else {
        return Status::Unknown;
    };
    if version > current {
        Status::Available(Release {
            tag: payload.tag_name,
            name: payload.name,
            version,
            url: payload.html_url,
            published_at: payload.published_at,
            notes: truncate_notes(&payload.body),
        })
    } else {
        Status::UpToDate
    }
}

/// Fetch the raw `releases/latest` JSON for `owner/repo` via `curl`. Bounded by
/// connection/total deadlines and a response-size cap, so a stalled or oversized
/// response fails promptly rather than hanging startup or consuming memory.
pub fn fetch_latest(repo: &str) -> Result<String> {
    if !is_valid_repo(repo) {
        return Err(Error::Request(format!("invalid repo slug: {repo:?}")));
    }
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let output = Command::new("curl")
        .args(curl_args(&url))
        .output()
        .map_err(|e| Error::Spawn(e.to_string()))?;
    if !output.status.success() {
        return Err(Error::Request(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    let body = String::from_utf8_lossy(&output.stdout).into_owned();
    if body.trim().is_empty() {
        return Err(Error::Parse("empty response".into()));
    }
    Ok(body)
}

/// curl argv for the release check: follow redirects, but bound connection
/// (10s), total time (20s), and response size (1 MiB).
fn curl_args(url: &str) -> Vec<String> {
    vec![
        "-sSL".into(),
        "--connect-timeout".into(),
        "10".into(),
        "--max-time".into(),
        "20".into(),
        "--max-filesize".into(),
        "1048576".into(),
        "-H".into(),
        "Accept: application/vnd.github+json".into(),
        "-H".into(),
        "User-Agent: asylum-update-check".into(),
        url.into(),
    ]
}

/// Whether `repo` is a plausible `owner/repo` slug: exactly one slash, no path
/// traversal, and only slug characters - so it cannot expand the request URL.
fn is_valid_repo(repo: &str) -> bool {
    let Some((owner, name)) = repo.split_once('/') else {
        return false;
    };
    let ok = |s: &str| {
        !s.is_empty()
            && !s.contains("..")
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    ok(owner) && ok(name)
}

/// Check `owner/repo` for a release newer than `current`. Network failures
/// surface as [`Status::Unknown`] via the caller; parse/compare is pure.
pub fn check(repo: &str, current: &str) -> Result<Status> {
    Ok(evaluate(current, &fetch_latest(repo)?))
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
