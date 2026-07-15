//! Design mode.
//!
//! Design mode lets you click any element in the embedded browser, attach a
//! note to it, and ship the batch to an agent. This crate provides the pure
//! halves of that:
//!
//! - [`INJECT_JS`] - a script injected into the web view at document start. In
//!   design mode it highlights the hovered element and, on click, captures the
//!   element's tag, a unique CSS selector, its `outerHTML`, computed styles, and
//!   text, then hands them to the host via `window.ipc.postMessage(...)`. It
//!   also draws numbered pin badges on annotated elements ([`pin_js`]).
//! - [`parse`] / [`Capture`] - parse that payload.
//! - [`Annotation`] / [`to_prompt`] / [`to_prompt_many`] - a capture plus the
//!   user's note, and the ready-to-send agent prompts built from them.
//!
//! The host (the gpui app) owns the wiring: install `INJECT_JS` via
//! `WebView::init_script`, toggle design mode with [`ENABLE_JS`]/[`DISABLE_JS`]
//! through `evaluate_script`, on `WebViewEvent::Message` call [`parse`], and
//! keep the page's pins matching the annotation list with [`pins_js`].

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

/// A captured element with the user's note attached - one design annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    pub capture: Capture,
    /// What should change about the element (may be empty).
    pub note: String,
}

/// Build one agent prompt from a batch of annotations: a numbered section per
/// element carrying the note, selector, and captured HTML/CSS context.
pub fn to_prompt_many(annotations: &[Annotation]) -> String {
    let mut out = String::from(
        "Apply the following UI changes. Each item was annotated on a live \
         preview of the app; the selector, HTML, and computed CSS identify the \
         element in the source.\n",
    );
    for (i, a) in annotations.iter().enumerate() {
        out.push_str(&format!("\n## {} — `{}`\n", i + 1, a.capture.selector));
        if !a.note.trim().is_empty() {
            out.push_str(&format!("\n{}\n", a.note.trim()));
        }
        if !a.capture.html.is_empty() {
            out.push_str(&format!("\nHTML:\n```html\n{}\n```\n", a.capture.html));
        }
        if !a.capture.css.is_empty() {
            out.push_str(&format!(
                "\nComputed CSS:\n```css\n{}\n```\n",
                a.capture.css
            ));
        }
    }
    out
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
/// `enable()` / `disable()`, posts a capture on click while enabled, and draws
/// numbered pin badges on annotated elements via `pin(selector, n)` /
/// `clearPins()` (kept in place across scroll and resize).
pub const INJECT_JS: &str = r#"
(function () {
  if (window.__asylumDesign) return;
  var on = false, last = null, pins = [];
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
  function place(p) {
    var el = document.querySelector(p.sel);
    if (!el) return;
    var r = el.getBoundingClientRect();
    p.node.style.left = (window.scrollX + r.left - 9) + 'px';
    p.node.style.top = (window.scrollY + r.top - 9) + 'px';
  }
  function repaint() { for (var i = 0; i < pins.length; i++) place(pins[i]); }
  window.addEventListener('scroll', repaint, true);
  window.addEventListener('resize', repaint);
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
    disable: function () { on = false; if (last) last.style.outline = ''; },
    pin: function (sel, n) {
      var node = document.createElement('div');
      node.textContent = String(n);
      node.style.cssText = 'position:absolute;z-index:2147483647;width:18px;' +
        'height:18px;border-radius:9px;background:#3b82f6;color:#fff;' +
        'font:bold 11px/18px sans-serif;text-align:center;pointer-events:none;' +
        'box-shadow:0 1px 4px rgba(0,0,0,.4);';
      document.body.appendChild(node);
      var p = { sel: sel, node: node };
      pins.push(p);
      place(p);
    },
    clearPins: function () {
      for (var i = 0; i < pins.length; i++) pins[i].node.remove();
      pins = [];
    }
  };
})();
"#;

/// JS that drops a numbered pin badge on the element `selector` matches. The
/// selector is JSON-escaped, so any capture's selector is safe to embed.
pub fn pin_js(selector: &str, n: usize) -> String {
    let sel = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".to_string());
    format!("window.__asylumDesign && window.__asylumDesign.pin({sel}, {n});")
}

/// JS that removes every pin badge.
pub const CLEAR_PINS_JS: &str = "window.__asylumDesign && window.__asylumDesign.clearPins();";

/// JS that redraws the page's pins to match `annotations` (numbered from 1) -
/// evaluate after removing an annotation or when a page finishes loading.
pub fn pins_js(annotations: &[Annotation]) -> String {
    let mut js = String::from(CLEAR_PINS_JS);
    for (i, a) in annotations.iter().enumerate() {
        js.push('\n');
        js.push_str(&pin_js(&a.capture.selector, i + 1));
    }
    js
}

/// Turn design mode on (call via `evaluate_script`).
pub const ENABLE_JS: &str = "window.__asylumDesign && window.__asylumDesign.enable();";
/// Turn design mode off.
pub const DISABLE_JS: &str = "window.__asylumDesign && window.__asylumDesign.disable();";

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
