use super::*;

#[test]
fn strips_markdown_headers() {
    assert_eq!(strip_markdown_line("# Big Header"), "Big Header");
    assert_eq!(strip_markdown_line("### Sub"), "Sub");
    assert_eq!(strip_markdown_line("No header here"), "No header here");
}

#[test]
fn strips_markdown_links_to_their_label() {
    assert_eq!(
        strip_markdown_line("See [the docs](https://example.com) for more."),
        "See the docs for more."
    );
    assert_eq!(strip_markdown_line("[a](x)[b](y)"), "ab");
}

#[test]
fn leaves_malformed_brackets_intact_without_panicking() {
    assert_eq!(strip_markdown_line("weird [unclosed"), "weird [unclosed");
    assert_eq!(
        strip_markdown_line("[no url here] rest"),
        "[no url here] rest"
    );
    // Nested brackets are not a link Asylum tries to parse; crude, not wrong.
    assert_eq!(strip_markdown_line("[a[b]c](url)"), "[a[b]c](url)");
}

#[test]
fn strip_markdown_line_is_utf8_safe() {
    assert_eq!(
        strip_markdown_line("# 日本語 [リンク](https://x)"),
        "日本語 リンク"
    );
}

#[test]
fn notes_preview_keeps_first_few_lines_plain_text() {
    let notes = "# Release v1.4\n\n- Fixed [a bug](https://x)\n- Added widgets\n\
                  - Improved startup\n- Something else\n- Sixth line dropped";
    let preview = notes_preview(notes);
    assert!(preview.contains("Fixed a bug"));
    assert!(!preview.contains('['));
    assert!(!preview.contains("Sixth line"));
}

#[test]
fn notes_preview_caps_very_long_single_line() {
    let notes = "a".repeat(2000);
    let preview = notes_preview(&notes);
    assert!(preview.chars().count() <= PREVIEW_CHARS + 1);
    assert!(preview.ends_with('…'));
}

#[test]
fn notes_preview_handles_empty_notes() {
    assert_eq!(notes_preview(""), "");
}

#[test]
fn update_body_embeds_the_disclaimer_and_url() {
    let body = update_body("Fixed things.", "https://example.com/release");
    assert!(body.contains("Fixed things."));
    assert!(body.contains(UPDATE_DISCLAIMER));
    assert!(body.ends_with("https://example.com/release"));
}

#[test]
fn update_body_handles_empty_notes() {
    let body = update_body("", "https://example.com/release");
    assert!(body.starts_with(UPDATE_DISCLAIMER));
    assert!(body.contains("https://example.com/release"));
}

#[test]
fn parse_update_body_round_trips_preview_and_url() {
    let body = update_body("Fixed a bug.\nAdded widgets.", "https://example.com/r/9");
    let (preview, url) = parse_update_body(&body);
    assert!(preview.contains("Fixed a bug."));
    assert_eq!(url.as_deref(), Some("https://example.com/r/9"));
}

#[test]
fn parse_update_body_is_defensive_on_unexpected_shapes() {
    let (preview, url) = parse_update_body("just some legacy text");
    assert_eq!(preview, "just some legacy text");
    assert_eq!(url, None);
}

#[test]
fn truncate_chars_is_utf8_safe_and_marks_truncation() {
    let text = "€".repeat(200);
    let truncated = truncate_chars(&text, 50);
    assert_eq!(truncated.chars().count(), 51); // 50 kept + the ellipsis mark
    assert!(truncated.ends_with('…'));

    let short = "short";
    assert_eq!(truncate_chars(short, 50), short);
}
