//! The command palette and quick-open overlays.

use gpui::prelude::*;
use gpui::Context;
use guise::prelude::*;

use crate::state::{Root, View};

impl Root {
    /// Build the command palette and quick-open overlays once. The palette lists
    /// view-switch and action commands; quick-open lists the project's files.
    pub fn ensure_palettes(&mut self, cx: &mut Context<Self>) {
        if self.palette.is_none() {
            let handle = cx.entity();
            let palette = cx.new(|cx| {
                let mut s = Spotlight::new(cx);
                for view in View::PRIMARY.iter().chain(View::MORE) {
                    let view = *view;
                    let h = handle.clone();
                    s = s.item(format!("Go to {}", view.label()), move |window, cx| {
                        h.update(cx, |root, cx| {
                            root.open_view(view, window, cx);
                            cx.notify();
                        });
                    });
                }
                let h = handle.clone();
                s = s.item("Run fan-out", move |window, cx| {
                    h.update(cx, |root, cx| {
                        root.run_fanout(window, cx);
                        cx.notify();
                    });
                });
                let h = handle.clone();
                s = s.item("Run checks", move |_, cx| {
                    h.update(cx, |root, cx| {
                        root.run_checks(cx);
                        cx.notify();
                    });
                });
                let h = handle.clone();
                s = s.item("Open selected run terminal", move |_, cx| {
                    h.update(cx, |root, cx| {
                        if let Some(id) = root.current_run_id() {
                            root.open_run_terminal(id);
                            cx.notify();
                        } else {
                            root.push_error("No run selected", "Select a run first.");
                        }
                    });
                });
                let h = handle.clone();
                s = s.item("Cancel selected run", move |_, cx| {
                    h.update(cx, |root, cx| {
                        if let Some(id) = root.current_run_id() {
                            root.cancel_run(id, cx);
                            cx.notify();
                        } else {
                            root.push_error("No run selected", "Select a run first.");
                        }
                    });
                });
                let h = handle.clone();
                s = s.item("Retry selected run", move |window, cx| {
                    h.update(cx, |root, cx| {
                        if let Some(id) = root.current_run_id() {
                            root.retry_run(id, window, cx);
                            cx.notify();
                        } else {
                            root.push_error("No run selected", "Select a run first.");
                        }
                    });
                });
                let h = handle.clone();
                s = s.item("Merge selected run", move |_, cx| {
                    h.update(cx, |root, cx| {
                        if let Some(id) = root.current_run_id() {
                            root.request_merge(id);
                            cx.notify();
                        } else {
                            root.push_error("No run selected", "Select a run first.");
                        }
                    });
                });
                let h = handle.clone();
                s = s.item("Open Settings", move |window, cx| {
                    h.update(cx, |root, cx| {
                        root.open_view(View::Settings, window, cx);
                        cx.notify();
                    });
                });
                let h = handle.clone();
                s = s.item("Open settings.json", move |_, cx| {
                    if let Err(error) = crate::menus::open_settings_file(cx) {
                        h.update(cx, |root, cx| {
                            root.push_error("Could not open settings", error);
                            cx.notify();
                        });
                    }
                });
                s.item("Toggle theme", move |_, cx| crate::theme::toggle(cx))
            });
            self.palette = Some(palette);
        }

        if self.quickopen.is_none() {
            let handle = cx.entity();
            let files = self.project_files();
            let quickopen = cx.new(|cx| {
                let mut s = Spotlight::new(cx);
                for name in files {
                    let h = handle.clone();
                    let file = name.clone();
                    s = s.item(name, move |_, cx| {
                        let file = file.clone();
                        h.update(cx, |root, cx| {
                            root.open_file(&file, cx);
                            cx.notify();
                        });
                    });
                }
                s
            });
            self.quickopen = Some(quickopen);
        }
    }
}
