//! Semantic activity detection from an agent's terminal output.
//!
//! A run's lifecycle [`status`](store::RunStatus) tells you the *process* is
//! alive; it does not tell you that agent #3 of five is **blocked waiting for
//! your input** while the others still churn. On a fan-out board that "which one
//! needs me right now" signal is exactly what is missing, so this module
//! classifies the live transcript into one of four states:
//!
//! - [`Activity::Blocked`] - stopped at an input prompt (a `(y/n)`, a selection
//!   menu, a password). The most actionable state, so it wins over the others.
//! - [`Activity::Working`] - actively thinking, editing, running a command.
//! - [`Activity::Done`] - printed a completion marker; awaiting review.
//! - [`Activity::Idle`] - initialised but showing no signal.
//!
//! Detection is a pure function over a snapshot of the recent output plus a set
//! of [`ActivityRules`] (generic defaults, with per-agent additions from
//! [`rules_for`]). It never inspects timing, so it stays trivially testable; the
//! host samples the pty and calls [`classify`].

/// The semantic state of a running agent, over and above its lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    /// Initialised but producing no recognisable signal.
    Idle,
    /// Actively thinking, editing, or running a command.
    Working,
    /// Stopped at a prompt, waiting for the user to answer.
    Blocked,
    /// Printed a completion marker; awaiting review.
    Done,
}

impl Activity {
    /// The stored/streamed token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::Blocked => "blocked",
            Self::Done => "done",
        }
    }

    /// Parse a token, defaulting to [`Activity::Idle`] on anything unknown.
    pub fn parse(s: &str) -> Self {
        match s {
            "working" => Self::Working,
            "blocked" => Self::Blocked,
            "done" => Self::Done,
            _ => Self::Idle,
        }
    }

    /// Classify `tail` for `agent_id` using that agent's rules. A convenience
    /// over [`classify`] + [`rules_for`].
    pub fn detect(agent_id: &str, tail: &str) -> Option<Activity> {
        classify(tail, &rules_for(agent_id))
    }
}

/// Substring markers for each state, matched case-insensitively against
/// ANSI-stripped output. Substrings (not regexes) keep the catalog readable and
/// the match cheap; an agent's live output changes shape often enough that broad
/// tokens age better than precise ones.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivityRules {
    /// Markers of a prompt awaiting user input.
    pub blocked: Vec<String>,
    /// Markers of a finished turn.
    pub done: Vec<String>,
    /// Markers of active work.
    pub working: Vec<String>,
}

impl ActivityRules {
    fn extend(&mut self, blocked: &[&str], done: &[&str], working: &[&str]) {
        self.blocked.extend(blocked.iter().map(|s| s.to_string()));
        self.done.extend(done.iter().map(|s| s.to_string()));
        self.working.extend(working.iter().map(|s| s.to_string()));
    }
}

/// How many trailing non-empty lines are considered "recent".
const RECENT: usize = 12;
/// A blocking prompt lives on the last line or two - scan a tight window so an
/// earlier `(y/n)` in scrollback does not read as a live prompt.
const BLOCKED_SCAN: usize = 4;
/// A completion marker likewise sits at the tail of the turn.
const DONE_SCAN: usize = 3;

/// Classify a transcript snapshot. Returns `None` when nothing matches, letting
/// the caller keep the prior state (or fall back to [`Activity::Idle`]).
///
/// Precedence is deliberate: a live input prompt ([`Activity::Blocked`]) is the
/// most useful thing to surface, so it is checked first, then completion, then
/// activity.
pub fn classify(tail: &str, rules: &ActivityRules) -> Option<Activity> {
    let clean = strip_ansi(tail);
    let lines: Vec<String> = clean
        .lines()
        .map(|l| l.trim().to_ascii_lowercase())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }
    let recent: Vec<&String> = lines.iter().rev().take(RECENT).collect();

    let hit = |window: usize, needles: &[String]| {
        recent[..window.min(recent.len())]
            .iter()
            .any(|line| needles.iter().any(|n| line.contains(n.as_str())))
    };

    if hit(BLOCKED_SCAN, &rules.blocked) {
        return Some(Activity::Blocked);
    }
    if hit(DONE_SCAN, &rules.done) {
        return Some(Activity::Done);
    }
    if hit(RECENT, &rules.working) {
        return Some(Activity::Working);
    }
    None
}

/// The generic rules, broad enough to cover most CLI agents out of the box.
pub fn default_rules() -> ActivityRules {
    let mut rules = ActivityRules::default();
    rules.extend(
        // blocked - waiting on the user
        &[
            "(y/n)",
            "[y/n]",
            "y/n)",
            "[y/n]",
            "(yes/no)",
            "yes/no",
            "? [y",
            "? (y",
            "press enter",
            "hit enter",
            "[enter]",
            "do you want",
            "would you like",
            "continue?",
            "proceed?",
            "confirm?",
            "overwrite?",
            "apply this",
            "apply changes?",
            "apply edit",
            "waiting for",
            "awaiting",
            "approve",
            "allow this",
            "permission to",
            "use arrow keys",
            "password:",
            "passphrase:",
            "enter your",
            "paste your",
            "(a)llow",
            "[a]llow",
            "❯",
            "› ",
        ],
        // done - turn finished
        &[
            "done",
            "completed",
            "finished",
            "all set",
            "✔",
            "✓",
            "success",
            "succeeded",
            "changes applied",
            "committed",
            "task complete",
            "no changes",
        ],
        // working - active
        &[
            "thinking",
            "working",
            "generating",
            "analyzing",
            "reasoning",
            "running",
            "executing",
            "building",
            "compiling",
            "editing",
            "writing",
            "reading",
            "searching",
            "applying",
            "processing",
            "fetching",
            "esc to interrupt",
            "tokens",
            "…",
            "⠋",
            "⠙",
            "⠹",
            "⠸",
            "⠼",
            "⠴",
            "⠦",
            "⠧",
            "⠇",
            "⠏",
        ],
    );
    rules
}

/// The rules for one agent: the [`default_rules`] plus any agent-specific
/// markers. Unknown ids fall back to the defaults.
pub fn rules_for(agent_id: &str) -> ActivityRules {
    let mut rules = default_rules();
    match agent_id {
        "claude-code" => rules.extend(
            &["1. yes", "2. no", "do you want to proceed", "❯ 1"],
            &["⏵⏵", "here's a summary"],
            &["✳", "· "],
        ),
        "aider" => rules.extend(
            &["(y)es/(n)o", "add these files", "create it?"],
            &["applied edit", "commit "],
            &[],
        ),
        "codex" => rules.extend(&["allow command", "run this command"], &["◆ done"], &[]),
        "gemini" => rules.extend(&["do you want to continue"], &[], &[]),
        _ => {}
    }
    rules
}

/// Strip ANSI/VT escape sequences so pattern matching sees plain text. Handles
/// CSI (`ESC [ … final`) and OSC (`ESC ] … BEL/ST`) plus a lone ESC.
fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('[') => {
                chars.next();
                // CSI: params/intermediates until a final byte in @..~.
                for f in chars.by_ref() {
                    if ('@'..='~').contains(&f) {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                // OSC: until BEL or ST (ESC \).
                while let Some(f) = chars.next() {
                    if f == '\u{7}' {
                        break;
                    }
                    if f == '\u{1b}' {
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                }
            }
            // A lone ESC or two-char sequence: drop the next char.
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

#[cfg(test)]
#[path = "../tests/activity.rs"]
mod tests;
