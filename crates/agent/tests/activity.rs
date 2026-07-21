use super::*;

#[test]
fn tokens_round_trip() {
    for a in [
        Activity::Idle,
        Activity::Working,
        Activity::Blocked,
        Activity::Done,
    ] {
        assert_eq!(Activity::parse(a.as_str()), a);
    }
    assert_eq!(Activity::parse("nonsense"), Activity::Idle);
}

#[test]
fn a_yes_no_prompt_reads_as_blocked() {
    let out = "Editing src/main.rs\nDo you want to proceed? (y/n)";
    assert_eq!(
        Activity::detect("claude-code", out),
        Some(Activity::Blocked)
    );
}

#[test]
fn a_selection_menu_reads_as_blocked() {
    let out = "Choose an option:\n❯ 1. Yes\n  2. No";
    assert_eq!(
        Activity::detect("claude-code", out),
        Some(Activity::Blocked)
    );
}

#[test]
fn blocked_wins_over_earlier_working_text() {
    let out = "Thinking...\nRunning tests\nApply changes? (y/n)";
    assert_eq!(classify(out, &default_rules()), Some(Activity::Blocked));
}

#[test]
fn a_completion_marker_reads_as_done() {
    let out = "Wrote 3 files\nAll set. Done.";
    assert_eq!(classify(out, &default_rules()), Some(Activity::Done));
}

#[test]
fn active_output_reads_as_working() {
    let out = "Analyzing the codebase\ngenerating a patch";
    assert_eq!(classify(out, &default_rules()), Some(Activity::Working));
}

#[test]
fn a_stale_done_up_in_scrollback_does_not_beat_fresh_work() {
    // "done" is far up; the fresh tail is active work.
    let mut lines = vec!["done with step one"];
    lines.extend(std::iter::repeat_n("reading files", 8));
    lines.push("executing command");
    let out = lines.join("\n");
    assert_eq!(classify(&out, &default_rules()), Some(Activity::Working));
}

#[test]
fn silence_yields_no_signal() {
    assert_eq!(classify("", &default_rules()), None);
    assert_eq!(classify("\n\n   \n", &default_rules()), None);
    assert_eq!(classify("just some banner text", &default_rules()), None);
}

#[test]
fn ansi_escapes_are_ignored() {
    // A spinner line wrapped in colour codes plus a cursor move.
    let out = "\u{1b}[2K\u{1b}[36m⠹\u{1b}[0m working on it";
    assert_eq!(classify(out, &default_rules()), Some(Activity::Working));

    let prompt = "\u{1b}[1mProceed?\u{1b}[0m (y/n)";
    assert_eq!(classify(prompt, &default_rules()), Some(Activity::Blocked));
}

#[test]
fn per_agent_rules_add_to_the_defaults() {
    // aider-specific prompt not in the generic set.
    let out = "Add these files to the chat? (Y)es/(N)o";
    assert_eq!(Activity::detect("aider", out), Some(Activity::Blocked));
    // Defaults still apply for a known agent.
    assert_eq!(
        Activity::detect("aider", "generating"),
        Some(Activity::Working)
    );
}

// The word "done" buried in prose ("not done yet") must not read as finished.
// A realistic Claude Code turn: a message line, a running tool call, a spinner.
#[test]
fn claude_prose_saying_not_done_yet_is_not_done() {
    let out = "⏺ I updated the parser but I'm not done yet - wiring tests next.\n  ⎿ Running cargo test (esc to interrupt)\n⠹ Testing";
    assert_eq!(
        Activity::detect("claude-code", out),
        Some(Activity::Working)
    );
}

// "on success we…" contains the "success" marker mid-line; still working.
#[test]
fn claude_prose_mentioning_success_is_not_done() {
    let out = "⏺ Adding a guard so that on success we skip the retry.\n  ⎿ Editing src/retry.rs";
    assert_eq!(
        Activity::detect("claude-code", out),
        Some(Activity::Working)
    );
}

// A bare prompt glyph from an idle TUI is not a request for input. These used
// to trip the generic blocked rules and ping the board for attention.
#[test]
fn a_bare_shell_prompt_is_not_blocked() {
    assert_eq!(Activity::detect("codex", "❯"), None);
    assert_eq!(Activity::detect("aider", "› "), None);
    assert_eq!(classify("❯", &default_rules()), None);
    assert_eq!(classify("› ", &default_rules()), None);
}

// A completed tool call (✓) is in the window, but a live spinner below it says
// the agent is mid-turn - the completion marker is stale, so work wins.
#[test]
fn a_finished_substep_under_a_live_spinner_is_still_working() {
    let out = "✓ Ran cargo build\n⠴ Working (esc to interrupt)";
    assert_eq!(Activity::detect("codex", out), Some(Activity::Working));
}

// Genuine end-of-turn markers, per agent, still classify as Done.
#[test]
fn true_completions_still_read_as_done() {
    // Claude Code closing a turn with its summary line.
    let claude = "Wrote src/parser.rs\n⏺ Here's a summary of the changes I made.";
    assert_eq!(
        Activity::detect("claude-code", claude),
        Some(Activity::Done)
    );
    // Aider: an applied edit followed by its commit line.
    let aider = "Applied edit to src/main.rs\nCommit a1b2c3d feat: add feature";
    assert_eq!(Activity::detect("aider", aider), Some(Activity::Done));
    // Codex's terminal completion glyph.
    assert_eq!(Activity::detect("codex", "◆ done"), Some(Activity::Done));
    // Generic: a check-marked completion with no spinner in the window.
    assert_eq!(
        classify("✓ Task complete", &default_rules()),
        Some(Activity::Done)
    );
}

// Genuine blocking prompts, per agent, still classify as Blocked.
#[test]
fn true_blocking_prompts_still_read_as_blocked() {
    let aider = "Add src/main.rs to the chat? (Y)es/(N)o [Yes]:";
    assert_eq!(Activity::detect("aider", aider), Some(Activity::Blocked));
    let codex = "Allow command: rm -rf build ? [y/n]";
    assert_eq!(Activity::detect("codex", codex), Some(Activity::Blocked));
    let claude = "Do you want to proceed?\n❯ 1. Yes\n  2. No";
    assert_eq!(
        Activity::detect("claude-code", claude),
        Some(Activity::Blocked)
    );
}
