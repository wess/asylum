use super::*;

#[test]
fn parses_a_capture_message() {
    let payload = r##"{"kind":"capture","selector":"#hero > button:nth-child(2)",
        "tag":"button","html":"<button>Buy</button>","css":"color: red;","text":"Buy"}"##;
    let cap = parse(payload).unwrap();
    assert_eq!(cap.tag, "button");
    assert_eq!(cap.selector, "#hero > button:nth-child(2)");
    assert_eq!(cap.text, "Buy");
}

#[test]
fn ignores_non_capture_messages() {
    assert!(parse(r#"{"kind":"other","selector":"x"}"#).is_none());
    assert!(parse("not json").is_none());
    assert!(parse(r#"{"kind":"capture","selector":""}"#).is_none());
}

#[test]
fn builds_agent_prompt() {
    let cap = Capture {
        selector: ".cta".into(),
        tag: "a".into(),
        html: "<a class=\"cta\">Go</a>".into(),
        css: "color: blue;".into(),
        text: "Go".into(),
    };
    let prompt = to_prompt(&cap, "Make this bigger and green");
    assert!(prompt.starts_with("Make this bigger and green"));
    assert!(prompt.contains("Target element: `.cta`"));
    assert!(prompt.contains("```html"));
    assert!(prompt.contains("<a class=\"cta\">Go</a>"));
    assert!(prompt.contains("```css"));
}

#[test]
fn inject_script_exposes_toggle_api() {
    assert!(INJECT_JS.contains("window.__asylumDesign"));
    assert!(INJECT_JS.contains("window.ipc.postMessage"));
    assert!(ENABLE_JS.contains("enable"));
    assert!(DISABLE_JS.contains("disable"));
}
