//! Provider sign-in probes for the account meter.
//!
//! Given a provider key from an account (`claude`, `codex`, `github`, …), a
//! probe runs that provider CLI's own cheap, non-interactive status command and
//! reads the result into [`Auth::SignedIn`] / [`Auth::SignedOut`] /
//! [`Auth::Unknown`]. A probe never spends tokens and never blocks the app: each
//! runs the child with stdin closed under a hard [`DEFAULT_TIMEOUT`], and a
//! child still alive at the deadline is killed and reported `Unknown` rather
//! than left to hang.
//!
//! The catalog and the interpretation are pure data plus free functions, so the
//! reading of a probe's output is unit-tested without spawning anything; only
//! [`run`] touches a process, covered by a deadline test and a live smoke test
//! gated on the CLI being installed.
//!
//! Where a provider's CLI has no safe non-interactive status command (Gemini
//! today, whose CLI is flag-only and interactive by default), its entry is
//! [`ProbeKind::Unsupported`] carrying a one-line reason rather than a guessed
//! probe that might hang or cost a request.

use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::doctor::find_program;

/// How long a single probe may run before it is killed and reported `Unknown`.
/// Status subcommands answer locally in well under a second; anything slower is
/// treated as inconclusive.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// How often the wait loop checks whether the child has exited.
const POLL: Duration = Duration::from_millis(25);

/// The sign-in verdict for a provider account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Auth {
    /// The CLI reports an authenticated session.
    SignedIn,
    /// The CLI reports no authenticated session.
    SignedOut,
    /// The probe could not decide, with a one-line reason for the tooltip.
    Unknown(String),
}

impl Auth {
    /// A short status label for the row.
    pub fn label(&self) -> &'static str {
        match self {
            Auth::SignedIn => "Signed in",
            Auth::SignedOut => "Signed out",
            Auth::Unknown(_) => "Unknown",
        }
    }

    /// The reason a probe was inconclusive, for a tooltip. `None` for a decided
    /// verdict.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Auth::Unknown(why) => Some(why.as_str()),
            _ => None,
        }
    }
}

/// How to read a probe command's output into an [`Auth`]. Markers are authored
/// lowercase and matched against stdout and stderr lowercased together.
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    /// Substrings that prove an authenticated session.
    pub signed_in: &'static [&'static str],
    /// Substrings that prove no session. Checked before [`Rule::signed_in`] so a
    /// "not logged in" line is never mistaken for "logged in".
    pub signed_out: &'static [&'static str],
    /// When no marker matches, whether a zero exit means signed in (and a
    /// non-zero exit signed out). `false` leaves an unrecognized result
    /// `Unknown` rather than guessing.
    pub trust_exit: bool,
}

/// A provider's probe: either a runnable status command or an explicit note that
/// no safe non-interactive check exists.
#[derive(Debug, Clone, Copy)]
pub enum ProbeKind {
    /// Run `program args` and read the result with `rule`.
    Command {
        program: &'static str,
        args: &'static [&'static str],
        rule: Rule,
    },
    /// No safe non-interactive status command; the reason to surface as
    /// `Unknown`.
    Unsupported(&'static str),
}

/// One provider in the catalog: a canonical key, the account-provider strings
/// that map to it, and its probe.
#[derive(Debug, Clone, Copy)]
pub struct Provider {
    pub key: &'static str,
    /// Account-provider strings (case-insensitive) that resolve to this entry.
    pub aliases: &'static [&'static str],
    pub probe: ProbeKind,
}

/// What the UI can do for a provider, without spawning anything.
#[derive(Debug, Clone)]
pub enum Kind {
    /// A runnable probe exists; run it on demand and read the cached verdict.
    Probeable,
    /// No live probe; show this fixed verdict (an `Unknown` with a reason).
    Static(Auth),
    /// Not in the catalog; make no sign-in claim.
    Absent,
}

/// The probe catalog. Only providers with a genuinely cheap, non-interactive,
/// no-cost auth check carry a [`ProbeKind::Command`]; the rest are
/// [`ProbeKind::Unsupported`] with an honest reason.
///
/// Verified against the CLIs' own help and output: `claude auth status` prints
/// `{"loggedIn": true, …}`; `gh auth status` prints "Logged in to …" (exit 0) or
/// "not logged in" (non-zero); `codex login status` prints "Logged in using …"
/// or "Not logged in".
pub const PROVIDERS: &[Provider] = &[
    Provider {
        key: "claude",
        aliases: &["claude", "claude-code", "anthropic"],
        probe: ProbeKind::Command {
            program: "claude",
            args: &["auth", "status"],
            // The JSON flag is authoritative; do not fall back to the exit code,
            // which can be zero even when the command merely ran.
            rule: Rule {
                signed_in: &["\"loggedin\": true"],
                signed_out: &["\"loggedin\": false"],
                trust_exit: false,
            },
        },
    },
    Provider {
        key: "github",
        aliases: &["github", "gh"],
        probe: ProbeKind::Command {
            program: "gh",
            args: &["auth", "status"],
            rule: Rule {
                signed_in: &["logged in to"],
                signed_out: &["not logged in", "not logged into"],
                trust_exit: true,
            },
        },
    },
    Provider {
        key: "codex",
        aliases: &["codex", "openai", "chatgpt"],
        probe: ProbeKind::Command {
            program: "codex",
            args: &["login", "status"],
            rule: Rule {
                signed_in: &["logged in"],
                signed_out: &["not logged in"],
                trust_exit: true,
            },
        },
    },
    Provider {
        key: "gemini",
        aliases: &["gemini", "google"],
        // The Gemini CLI is flag-only and starts an interactive session by
        // default; it exposes no status/auth subcommand to probe safely.
        probe: ProbeKind::Unsupported("no non-interactive status command"),
    },
];

/// Look up a provider by account key, case-insensitively, matching any alias.
pub fn find(provider: &str) -> Option<&'static Provider> {
    let want = provider.trim();
    if want.is_empty() {
        return None;
    }
    PROVIDERS
        .iter()
        .find(|p| p.aliases.iter().any(|a| a.eq_ignore_ascii_case(want)))
}

/// What the UI can offer for a provider, decided purely (no spawning): a live
/// probe, a fixed verdict, or nothing.
pub fn kind(provider: &str) -> Kind {
    match find(provider) {
        None => Kind::Absent,
        Some(p) => match &p.probe {
            ProbeKind::Command { .. } => Kind::Probeable,
            ProbeKind::Unsupported(reason) => Kind::Static(Auth::Unknown((*reason).to_string())),
        },
    }
}

/// Probe a provider account's sign-in state using [`DEFAULT_TIMEOUT`]. Blocking;
/// the app hands this to a background task.
pub fn check(provider: &str) -> Auth {
    check_with_timeout(provider, DEFAULT_TIMEOUT)
}

/// [`check`] with an explicit deadline.
pub fn check_with_timeout(provider: &str, timeout: Duration) -> Auth {
    let Some(p) = find(provider) else {
        return Auth::Unknown(format!("no sign-in probe for `{}`", provider.trim()));
    };
    let (program, args, rule) = match &p.probe {
        ProbeKind::Unsupported(reason) => return Auth::Unknown((*reason).to_string()),
        ProbeKind::Command {
            program,
            args,
            rule,
        } => (*program, *args, rule),
    };
    if find_program(program).is_none() {
        return Auth::Unknown(format!("`{program}` is not installed"));
    }
    match run(program, args, timeout) {
        Run::Ran {
            success,
            stdout,
            stderr,
        } => interpret(rule, success, &stdout, &stderr),
        Run::Timeout => Auth::Unknown(format!(
            "`{program}` did not answer within {}s",
            timeout.as_secs().max(1)
        )),
        Run::SpawnFailed(e) => Auth::Unknown(format!("`{program}` could not start: {e}")),
    }
}

/// Pure interpretation of a probe result. Separated from [`run`] so every rule
/// is tested without spawning. Markers in the catalog are lowercase; the output
/// is lowercased before matching.
pub fn interpret(rule: &Rule, success: bool, stdout: &str, stderr: &str) -> Auth {
    let haystack = format!("{stdout}\n{stderr}").to_lowercase();
    if rule.signed_out.iter().any(|m| haystack.contains(*m)) {
        return Auth::SignedOut;
    }
    if rule.signed_in.iter().any(|m| haystack.contains(*m)) {
        return Auth::SignedIn;
    }
    if rule.trust_exit {
        return if success {
            Auth::SignedIn
        } else {
            Auth::SignedOut
        };
    }
    Auth::Unknown("could not read the CLI's sign-in state".to_string())
}

/// The outcome of spawning a probe command.
enum Run {
    /// The child exited; carries whether it exited zero and its captured output.
    Ran {
        success: bool,
        stdout: String,
        stderr: String,
    },
    /// The child was still running at the deadline and was killed.
    Timeout,
    /// The child could not be launched.
    SpawnFailed(String),
}

/// Spawn `program args` with stdin closed and stdout/stderr captured, waiting up
/// to `timeout`. A child still running at the deadline is killed and reported as
/// [`Run::Timeout`]. Output pipes are drained on their own threads so a chatty
/// child can never deadlock the wait. This is the only function here that
/// touches a process.
fn run(program: &str, args: &[&str], timeout: Duration) -> Run {
    let mut child = match Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => return Run::SpawnFailed(e.to_string()),
    };

    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                thread::sleep(POLL);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };

    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    match status {
        Some(status) => Run::Ran {
            success: status.success(),
            stdout,
            stderr,
        },
        None => Run::Timeout,
    }
}

/// Read a child pipe to end on its own thread so a full pipe buffer never blocks
/// the wait loop. Once the child is killed or exits, its write end closes and
/// the read returns.
fn drain(pipe: Option<impl Read + Send + 'static>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_string(&mut buf);
        }
        buf
    })
}

#[cfg(test)]
#[path = "../tests/probe.rs"]
mod tests;
