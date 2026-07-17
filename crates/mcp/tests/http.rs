use super::*;

#[test]
fn content_length_parsing_is_strict() {
    assert_eq!(parse_content_length("10"), Some(10));
    // A repeated, consistent value is accepted (some proxies duplicate it).
    assert_eq!(parse_content_length("10, 10"), Some(10));
    // Inconsistent duplicates are a smuggling risk - reject.
    assert_eq!(parse_content_length("10, 11"), None);
    // A leading `+` is looser than RFC 7230's 1*DIGIT - reject.
    assert_eq!(parse_content_length("+10"), None);
    assert_eq!(parse_content_length("ten"), None);
    assert_eq!(parse_content_length(""), None);
}

#[test]
fn reason_phrases_cover_what_the_gateway_emits() {
    assert_eq!(reason_phrase(200), "OK");
    assert_eq!(reason_phrase(202), "Accepted");
    assert_eq!(reason_phrase(401), "Unauthorized");
    assert_eq!(reason_phrase(404), "Not Found");
    assert_eq!(reason_phrase(405), "Method Not Allowed");
    assert_eq!(reason_phrase(429), "Too Many Requests");
}

#[test]
fn find_subsequence_locates_the_header_terminator() {
    assert_eq!(find_subsequence(b"ab\r\n\r\ncd", b"\r\n\r\n"), Some(2));
    assert_eq!(find_subsequence(b"abcd", b"\r\n\r\n"), None);
}

#[test]
fn response_builders_set_the_content_type() {
    assert_eq!(
        Response::json(200, "{}".into()).content_type,
        "application/json"
    );
    assert!(Response::text(404, "x")
        .content_type
        .starts_with("text/plain"));
}
