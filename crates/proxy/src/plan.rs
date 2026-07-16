//! Pure request planning for the secrets proxy - resolve `/<upstream>/<path>`
//! into a concrete forward with the secret already injected. No I/O, so the
//! whole thing is unit-tested.

use config::Upstream;

/// A planned forward: the target URL and the single secret-bearing header to
/// attach. `header_value` already contains the resolved secret; the destination
/// host is fixed by the upstream's `base_url`, so the secret can only ever reach
/// that host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub url: String,
    pub header_name: String,
    pub header_value: String,
}

/// Why a request could not be planned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// No upstream is named in the path (`/`).
    BadPath,
    /// No configured upstream matches the name in the caller's scope.
    UnknownUpstream(String),
    /// The upstream's secret has no value in the (project or global) keep.
    MissingSecret(String),
    /// The upstream's `base_url` is not an http(s) URL.
    BadBaseUrl(String),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::BadPath => write!(f, "expected /<upstream>/<path>"),
            PlanError::UnknownUpstream(n) => write!(f, "no such upstream: {n}"),
            PlanError::MissingSecret(s) => write!(f, "secret not in keep: {s}"),
            PlanError::BadBaseUrl(u) => write!(f, "upstream base_url must be http(s): {u}"),
        }
    }
}

const DEFAULT_HEADER: &str = "Authorization";
const DEFAULT_FORMAT: &str = "Bearer {secret}";

/// Plan the forward for an agent request `path` like `/openai/v1/chat/...` made
/// from `project` (0 = global). A project-scoped upstream/secret overrides a
/// global one of the same name. `resolve` returns a secret's value by name
/// (backed by the keep, already scoped to `project`).
pub fn plan(
    path: &str,
    upstreams: &[Upstream],
    project: i64,
    resolve: impl Fn(&str) -> Option<String>,
) -> Result<Plan, PlanError> {
    let (pathpart, query) = match path.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (path, None),
    };
    let trimmed = pathpart.trim_start_matches('/');
    let (name, rest) = match trimmed.split_once('/') {
        Some((n, r)) => (n, r),
        None => (trimmed, ""),
    };
    if name.is_empty() {
        return Err(PlanError::BadPath);
    }

    // A project-scoped upstream wins over a global one of the same name.
    let up = select_upstream(upstreams, name, project)
        .ok_or_else(|| PlanError::UnknownUpstream(name.to_string()))?;

    let base = up.base_url.trim_end_matches('/');
    if !(base.starts_with("https://") || base.starts_with("http://")) {
        return Err(PlanError::BadBaseUrl(up.base_url.clone()));
    }

    let secret = resolve(&up.secret)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| PlanError::MissingSecret(up.secret.clone()))?;

    let header_name = non_empty(&up.header).unwrap_or(DEFAULT_HEADER).to_string();
    let format = non_empty(&up.format).unwrap_or(DEFAULT_FORMAT);
    let header_value = format.replace("{secret}", &secret);

    let mut url = if rest.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{rest}")
    };
    if let Some(q) = query {
        url.push('?');
        url.push_str(q);
    }

    Ok(Plan {
        url,
        header_name,
        header_value,
    })
}

/// Find the upstream named `name` visible to `project`: prefer a project-scoped
/// one (`project == project`), else the global one (`project == 0`).
fn select_upstream<'a>(
    upstreams: &'a [Upstream],
    name: &str,
    project: i64,
) -> Option<&'a Upstream> {
    let matches =
        |u: &&Upstream, scope: i64| u.name.eq_ignore_ascii_case(name) && u.project == scope;
    upstreams
        .iter()
        .find(|u| matches(u, project))
        .or_else(|| upstreams.iter().find(|u| matches(u, 0)))
}

fn non_empty(s: &str) -> Option<&str> {
    let t = s.trim();
    (!t.is_empty()).then_some(t)
}

#[cfg(test)]
#[path = "../tests/plan.rs"]
mod tests;
