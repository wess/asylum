//! The browser / preview design surface: an embedded web view with element
//! annotation. Toggle design mode, click an element, attach a note, collect
//! numbered pins, and ship the batch to an agent - the signature design flow.

use gpui::prelude::*;
use gpui::{div, px, Entity, IntoElement, SharedString};
use guise::prelude::*;
use guise::Switch;

use crate::state::Root;

/// The whole surface: a design toolbar, the collected annotations, and the
/// web view. Used by both the Browser and Preview tabs.
pub fn design_surface(
    webview: Entity<guise::WebView>,
    enabled: bool,
    pending: Option<designmode::Capture>,
    annotations: Vec<designmode::Annotation>,
    note: Entity<guise::TextInput>,
    handle: Entity<Root>,
) -> impl IntoElement {
    let strip = (!annotations.is_empty())
        .then(|| annotation_strip(webview.clone(), annotations, handle.clone()));
    div()
        .flex()
        .flex_col()
        .size_full()
        .child(toolbar(webview.clone(), enabled, pending, note, handle))
        .children(strip)
        .child(div().flex_1().overflow_hidden().child(webview))
}

/// The toolbar: the design-mode switch, then either the pending capture's
/// note row or a hint.
fn toolbar(
    webview: Entity<guise::WebView>,
    enabled: bool,
    pending: Option<designmode::Capture>,
    note: Entity<guise::TextInput>,
    handle: Entity<Root>,
) -> impl IntoElement {
    // Ids are scoped by the web view so a Browser and a Preview pane (or two
    // Browser tabs) never collide in gpui's element-state map.
    let wid = webview.entity_id().as_u64();
    let mut bar = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .px(px(8.0))
        .py(px(6.0));

    // The switch drives the injected script and tracks the state on Root, so
    // a page navigation can re-assert it (see `watch_design_messages`).
    let wv = webview.clone();
    let toggle = handle.clone();
    bar = bar.child(
        Switch::new(SharedString::from(format!("design-mode-{wid}")))
            .checked(enabled)
            .size(Size::Sm)
            .label("Design mode")
            .on_change(move |_, _, cx| {
                let on = !enabled;
                wv.read(cx).evaluate_script(if on {
                    designmode::ENABLE_JS
                } else {
                    designmode::DISABLE_JS
                });
                let id = wv.entity_id();
                toggle.update(cx, |root, cx| {
                    if on {
                        root.design_enabled.insert(id);
                    } else {
                        root.design_enabled.remove(&id);
                        root.pending_capture = None;
                    }
                    cx.notify();
                });
            }),
    );

    match pending {
        Some(capture) => {
            let note_read = note.clone();
            let attach = handle.clone();
            let wv = webview.clone();
            let dismiss = handle;
            bar.child(
                Badge::new(SharedString::from(capture.selector))
                    .color(ColorName::Blue)
                    .variant(Variant::Light),
            )
            .child(div().flex_1().child(note))
            .child(
                Button::new(SharedString::from(format!("attach-note-{wid}")), "Attach note")
                    .size(Size::Xs)
                    .variant(Variant::Filled)
                    .on_click(move |_, _, cx| {
                        let text = note_read.read(cx).text();
                        let mut pinned = None;
                        attach.update(cx, |root, cx| {
                            pinned = root.attach_design_note(&text);
                            cx.notify();
                        });
                        if let Some((selector, n)) = pinned {
                            wv.read(cx).evaluate_script(&designmode::pin_js(&selector, n));
                        }
                        note_read.update(cx, |n, cx| n.set_text("", cx));
                    }),
            )
            .child(
                Button::new(SharedString::from(format!("dismiss-note-{wid}")), "Dismiss")
                    .size(Size::Xs)
                    .variant(Variant::Subtle)
                    .on_click(move |_, _, cx| {
                        dismiss.update(cx, |root, cx| {
                            root.pending_capture = None;
                            cx.notify();
                        });
                    }),
            )
        }
        None => bar.child(
            Text::new(if enabled {
                "Click an element to annotate it"
            } else {
                "Turn on design mode to annotate elements"
            })
            .size(Size::Xs)
            .dimmed(),
        ),
    }
}

/// The collected annotations: one numbered row per pin (with remove), and the
/// button that ships the batch to an agent.
fn annotation_strip(
    webview: Entity<guise::WebView>,
    annotations: Vec<designmode::Annotation>,
    handle: Entity<Root>,
) -> impl IntoElement {
    let wid = webview.entity_id().as_u64();
    let mut list = div().flex().flex_col().gap_1().px(px(8.0)).pb(px(6.0));
    for (i, a) in annotations.iter().enumerate() {
        let remove = handle.clone();
        let wv = webview.clone();
        let label = if a.note.is_empty() {
            a.capture.selector.clone()
        } else {
            format!("{} — {}", a.capture.selector, a.note)
        };
        list = list.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    Badge::new(SharedString::from(format!("{}", i + 1)))
                        .color(ColorName::Blue)
                        .variant(Variant::Light),
                )
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .child(Text::new(SharedString::from(label)).size(Size::Xs)),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("design-x-{wid}-{i}")))
                        .px(px(4.0))
                        .cursor_pointer()
                        .child(Text::new("×").size(Size::Xs).dimmed())
                        .on_click(move |_, _, cx| {
                            let mut js = String::new();
                            remove.update(cx, |root, cx| {
                                root.remove_design_annotation(i);
                                js = designmode::pins_js(&root.design_annotations);
                                cx.notify();
                            });
                            wv.read(cx).evaluate_script(&js);
                        }),
                ),
        );
    }

    let send = handle;
    let wv = webview;
    let n = annotations.len();
    list.child(
        div().flex().flex_row().pt(px(2.0)).child(
            Button::new(
                SharedString::from(format!("send-design-{wid}")),
                SharedString::from(format!(
                    "Send {n} annotation{} to agent",
                    if n == 1 { "" } else { "s" }
                )),
            )
            .size(Size::Xs)
            .variant(Variant::Filled)
            .on_click(move |_, _, cx| {
                send.update(cx, |root, cx| {
                    root.send_design_to_agent();
                    cx.notify();
                });
                wv.read(cx).evaluate_script(designmode::CLEAR_PINS_JS);
            }),
        ),
    )
}
