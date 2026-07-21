//! Design mode: element captures, numbered annotations, and shipping the
//! batch to an agent.

use gpui::Context;

use crate::state::{now, Root};
use crate::workspace::TabKind;

impl Root {
    /// Route a web view's design-mode traffic: a capture becomes the pending
    /// annotation, and every page load re-asserts the design-mode toggle and
    /// redraws the pins (navigation wipes the page's state, not ours).
    pub(super) fn watch_design_messages(
        &mut self,
        wv: &gpui::Entity<guise::WebView>,
        cx: &mut Context<Self>,
    ) {
        cx.subscribe(
            wv,
            |root, wv, event: &guise::WebViewEvent, cx| match event {
                guise::WebViewEvent::Message(payload) => {
                    if let Some(capture) = designmode::parse(payload) {
                        root.pending_capture = Some(capture);
                        cx.notify();
                    }
                }
                guise::WebViewEvent::LoadFinished => {
                    if root.design_enabled.contains(&wv.entity_id()) {
                        wv.read(cx).evaluate_script(designmode::ENABLE_JS);
                    }
                    if !root.design_annotations.is_empty() {
                        wv.read(cx)
                            .evaluate_script(&designmode::pins_js(&root.design_annotations));
                    }
                }
                _ => {}
            },
        )
        .detach();
    }

    /// Attach a note to the pending capture, making it a numbered annotation.
    /// Returns the new annotation's (selector, number) so the view can pin it.
    pub fn attach_design_note(&mut self, note: &str) -> Option<(String, usize)> {
        let capture = self.pending_capture.take()?;
        let selector = capture.selector.clone();
        self.design_annotations.push(designmode::Annotation {
            capture,
            note: note.trim().to_string(),
        });
        Some((selector, self.design_annotations.len()))
    }

    /// Drop a design annotation (the view renumbers the pins via `pins_js`).
    pub fn remove_design_annotation(&mut self, index: usize) {
        if index < self.design_annotations.len() {
            self.design_annotations.remove(index);
        }
    }

    /// Ship the collected design annotations to an agent as a new task, then
    /// switch to the Tasks board.
    pub fn send_design_to_agent(&mut self) {
        if self.design_annotations.is_empty() {
            return;
        }
        let Some(pid) = self.project_id else {
            return;
        };
        let prompt = designmode::to_prompt_many(&self.design_annotations);
        let title = match self.design_annotations.as_slice() {
            [a] => format!("Design: {}", a.capture.selector),
            many => format!("Design: {} annotations", many.len()),
        };
        if let Ok(task) = self.db.create_task(pid, &title, &prompt, now()) {
            self.task_id = Some(task.id);
            self.design_annotations.clear();
            self.pending_capture = None;
            self.open_kind(TabKind::Tasks);
        }
    }
}
