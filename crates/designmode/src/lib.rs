//! Design mode.
//!
//! Design mode lets you click any element in the embedded browser and
//! sends its HTML, CSS, and a screenshot to an agent. This crate provides the
//! two pure halves of that:
//!
//! - [`INJECT_JS`] — a script injected into the web view at document start. In
//!   design mode it highlights the hovered element and, on click, captures the
//!   element's tag, a unique CSS selector, its `outerHTML`, computed styles, and
//!   text, then hands them to the host via `window.ipc.postMessage(...)`.
//! - [`parse`] / [`Capture`] / [`to_prompt`] — parse that payload and turn a
//!   capture into a ready-to-send agent prompt.
//!
//! The host (the gpui app) owns the wiring: install `INJECT_JS` via
//! `WebView::init_script`, toggle design mode with [`ENABLE_JS`]/[`DISABLE_JS`]
//! through `evaluate_script`, and on `WebViewEvent::Message` call [`parse`].

use serde::Deserialize;

/// A captured element from the page.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Capture {
    /// A CSS selector that uniquely identifies the element.
    pub selector: String,
    /// The element's tag name (lowercase).
    #[serde(default)]
    pub tag: String,
    /// The element's `outerHTML` (truncated by the script).
    #[serde(default)]
    pub html: String,
    /// Key computed styles as `prop: value;` declarations.
    #[serde(default)]
    pub css: String,
    /// The element's visible text (truncated).
    #[serde(default)]
    pub text: String,
}

/// A design-mode message from the page. `kind` is `"capture"` for a click.
#[derive(Debug, Clone, Deserialize)]
struct Message {
    #[serde(default)]
    kind: String,
    #[serde(flatten)]
    capture: Capture,
}

/// Parse a `window.ipc.postMessage` payload into a [`Capture`]. Returns `None`
/// for non-capture messages or malformed payloads.
pub fn parse(payload: &str) -> Option<Capture> {
    let msg: Message = serde_json::from_str(payload).ok()?;
    if msg.kind != "capture" {
        return None;
    }
    if msg.capture.selector.is_empty() {
        return None;
    }
    Some(msg.capture)
}

/// Build an agent prompt from a capture and the user's instruction.
pub fn to_prompt(capture: &Capture, instruction: &str) -> String {
    let mut out = String::new();
    if !instruction.trim().is_empty() {
        out.push_str(instruction.trim());
        out.push_str("\n\n");
    }
    out.push_str(&format!("Target element: `{}`\n\n", capture.selector));
    if !capture.html.is_empty() {
        out.push_str("HTML:\n```html\n");
        out.push_str(&capture.html);
        out.push_str("\n```\n\n");
    }
    if !capture.css.is_empty() {
        out.push_str("Computed CSS:\n```css\n");
        out.push_str(&capture.css);
        out.push_str("\n```\n");
    }
    out
}

/// Script injected at document start. It exposes `window.__asylumDesign` with
/// `enable()` / `disable()`, and posts a capture on click while enabled.
pub const INJECT_JS: &str = r#"
(function () {
  if (window.__asylumDesign) return;
  var on = false, last = null;
  function selector(el) {
    if (el.id) return '#' + el.id;
    var parts = [];
    while (el && el.nodeType === 1 && parts.length < 6) {
      var part = el.tagName.toLowerCase();
      if (el.className && typeof el.className === 'string') {
        var c = el.className.trim().split(/\s+/).slice(0, 2).join('.');
        if (c) part += '.' + c;
      }
      var parent = el.parentNode;
      if (parent) {
        var i = Array.prototype.indexOf.call(parent.children, el) + 1;
        part += ':nth-child(' + i + ')';
      }
      parts.unshift(part);
      el = el.parentNode;
    }
    return parts.join(' > ');
  }
  function styles(el) {
    var cs = getComputedStyle(el);
    var props = ['display','position','width','height','margin','padding',
      'color','background-color','font-size','font-family','border','flex'];
    return props.map(function (p) { return p + ': ' + cs.getPropertyValue(p) + ';'; }).join('\n');
  }
  function capture(el) {
    return JSON.stringify({
      kind: 'capture',
      selector: selector(el),
      tag: el.tagName.toLowerCase(),
      html: (el.outerHTML || '').slice(0, 4000),
      css: styles(el),
      text: (el.innerText || '').slice(0, 500)
    });
  }
  document.addEventListener('mouseover', function (e) {
    if (!on) return;
    if (last) last.style.outline = '';
    last = e.target;
    last.style.outline = '2px solid #3b82f6';
  }, true);
  document.addEventListener('click', function (e) {
    if (!on) return;
    e.preventDefault(); e.stopPropagation();
    try { window.ipc.postMessage(capture(e.target)); } catch (err) {}
  }, true);
  window.__asylumDesign = {
    enable: function () { on = true; },
    disable: function () { on = false; if (last) last.style.outline = ''; }
  };
})();
"#;

/// Turn design mode on (call via `evaluate_script`).
pub const ENABLE_JS: &str = "window.__asylumDesign && window.__asylumDesign.enable();";
/// Turn design mode off.
pub const DISABLE_JS: &str = "window.__asylumDesign && window.__asylumDesign.disable();";

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
