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
