use super::*;

#[test]
fn strips_line_comments() {
    let src = "{ \"a\": 1 // trailing\n}";
    let out = strip(src);
    assert!(!out.contains("trailing"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&out).unwrap()["a"],
        1
    );
}

#[test]
fn strips_block_comments() {
    let src = "{ /* hi */ \"a\": 2 }";
    let out = strip(src);
    assert!(!out.contains("hi"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&out).unwrap()["a"],
        2
    );
}

#[test]
fn leaves_comment_markers_inside_strings() {
    let src = "{ \"url\": \"http://x/y\" }";
    let out = strip(src);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&out).unwrap()["url"],
        "http://x/y"
    );
}

#[test]
fn preserves_unicode() {
    let src = "{ \"name\": \"café ☕\" } // note";
    let out = strip(src);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&out).unwrap()["name"],
        "café ☕"
    );
}
