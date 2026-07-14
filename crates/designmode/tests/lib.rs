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

fn annotation(selector: &str, note: &str) -> Annotation {
    Annotation {
        capture: Capture {
            selector: selector.into(),
            tag: "div".into(),
            html: format!("<div class=\"{selector}\"></div>"),
            css: "display: block;".into(),
            text: String::new(),
        },
        note: note.into(),
    }
}

#[test]
fn builds_batch_prompt_with_numbered_sections() {
    let prompt = to_prompt_many(&[
        annotation(".hero", "Make the heading larger"),
        annotation(".cta", ""),
    ]);
    assert!(prompt.contains("## 1 — `.hero`"));
    assert!(prompt.contains("Make the heading larger"));
    assert!(prompt.contains("## 2 — `.cta`"));
    assert!(prompt.contains("```html"));
    assert!(prompt.contains("```css"));
}

#[test]
fn batch_prompt_skips_empty_parts() {
    let mut a = annotation(".x", "");
    a.capture.html.clear();
    a.capture.css.clear();
    let prompt = to_prompt_many(&[a]);
    assert!(prompt.contains("## 1 — `.x`"));
    assert!(!prompt.contains("```html"));
    assert!(!prompt.contains("```css"));
}

#[test]
fn pin_js_escapes_the_selector() {
    let js = pin_js("div[title=\"a\\\"b\"]", 3);
    assert!(js.contains(".pin("));
    assert!(js.ends_with(", 3);"));
    // The selector arrives as one JSON string literal - quotes stay escaped.
    assert!(js.contains(r#""div[title=\"a\\\"b\"]""#));
}

#[test]
fn pins_js_clears_then_renumbers() {
    let js = pins_js(&[annotation(".a", ""), annotation(".b", "")]);
    assert!(js.starts_with(CLEAR_PINS_JS));
    assert!(js.contains(r#".pin(".a", 1)"#));
    assert!(js.contains(r#".pin(".b", 2)"#));
    assert_eq!(pins_js(&[]), CLEAR_PINS_JS);
}

#[test]
fn inject_script_exposes_pin_api() {
    assert!(INJECT_JS.contains("pin: function"));
    assert!(INJECT_JS.contains("clearPins"));
}
