use super::*;

#[test]
fn player_is_accessible_and_does_not_autoplay() {
    let html = player_html();
    assert!(html.contains("controls playsinline"));
    assert!(!html.contains(" autoplay"));
    assert!(html.contains("track.default = enabled"));
    assert!(html.contains("Read the full transcript"));
    assert!(html.contains("data-time=\"307.262\""));
}

#[test]
fn transcript_is_escaped_and_structured() {
    let html = transcript_html("# Title\n\n## A & B\n\nUse <this> safely.");
    assert_eq!(html, "<h2>A &amp; B</h2><p>Use &lt;this&gt; safely.</p>");
}

#[test]
fn media_fingerprint_changes_with_content() {
    assert_ne!(fingerprint(&[b"video-a"]), fingerprint(&[b"video-b"]));
    assert_ne!(
        fingerprint(&[b"video", b"captions-a"]),
        fingerprint(&[b"video", b"captions-b"])
    );
    assert_eq!(fingerprint(&[b"video-a"]), fingerprint(&[b"video-a"]));
}
