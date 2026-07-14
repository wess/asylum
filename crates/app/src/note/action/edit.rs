use gpui::{AppContext as _, Context, Window};
use guise::editor::EditorEvent;
use guise::TextInputEvent;

use crate::state::Root;

impl Root {
    /// Build the note entities once and load the selected project's vault when
    /// the project changes.
    pub fn ensure_notes(&mut self, cx: &mut Context<Self>) {
        if self.note.editor.is_none() {
            let editor = cx.new(|cx| {
                guise::Editor::new(cx)
                    .line_numbers(false)
                    .placeholder("Start writing. Type [[ to link another note.")
            });
            cx.subscribe(&editor, |root, editor, event: &EditorEvent, cx| {
                if let EditorEvent::Change(content) = event {
                    let fragment = completion(editor.read(cx));
                    root.note_changed(content, fragment, cx);
                }
            })
            .detach();
            self.note.editor = Some(editor);
        }
        if self.note.search.is_none() {
            let input = cx.new(|cx| guise::TextInput::new(cx).placeholder("Filter notes"));
            cx.subscribe(&input, |root, _input, event: &TextInputEvent, cx| {
                let query = match event {
                    TextInputEvent::Change(query) | TextInputEvent::Submit(query) => query,
                };
                root.note.query = query.clone();
                cx.notify();
            })
            .detach();
            self.note.search = Some(input);
        }
        if self.note.title.is_none() {
            let input = cx.new(|cx| guise::TextInput::new(cx).placeholder("Note title"));
            cx.subscribe(&input, |root, _input, event: &TextInputEvent, cx| {
                if matches!(event, TextInputEvent::Submit(_)) {
                    root.rename_current_note(cx);
                }
            })
            .detach();
            self.note.title = Some(input);
        }
        if self.note.preview.is_none() {
            let preview = cx.new(|cx| {
                guise::WebView::new(cx).html(preview::html_markdown("# No note selected"))
            });
            cx.subscribe(
                &preview,
                |root, _preview, event: &guise::WebViewEvent, cx| {
                    if let guise::WebViewEvent::UrlChanged(url) = event {
                        if let Some(target) = url.strip_prefix("asylum://note/") {
                            root.open_wiki_link(target, cx);
                        }
                    }
                },
            )
            .detach();
            self.note.preview = Some(preview);
        }
        if self.note.project_id != self.project_id {
            self.load_project_notes(cx);
        } else if self.note.saved {
            self.sync_note_entities(cx);
        }
    }

    pub fn set_note_view(&mut self, mode: super::super::Mode, cx: &mut Context<Self>) {
        self.note.view = mode;
        if mode == super::super::Mode::Edit {
            self.hide_note_preview(cx);
        }
    }

    pub fn set_note_panel(&mut self, panel: super::super::Panel, cx: &mut Context<Self>) {
        self.note.panel = panel;
        if panel != super::super::Panel::Write {
            self.hide_note_preview(cx);
        }
    }

    pub fn hide_note_preview(&self, cx: &mut Context<Self>) {
        if let Some(preview) = self.note.preview.clone() {
            preview.update(cx, |preview, _cx| preview.set_visible(false));
        }
    }

    pub fn insert_note_link(&mut self, title: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.note.editor.clone() else {
            return;
        };
        let fragment = completion(editor.read(cx)).unwrap_or_default();
        editor.update(cx, |editor, cx| {
            editor.edit(window, cx, |model| {
                for _ in fragment.chars() {
                    model.backspace();
                }
                model.insert(title);
                model.insert("]]");
            });
        });
        self.note.suggestions.clear();
    }

    fn open_wiki_link(&mut self, target: &str, cx: &mut Context<Self>) {
        let target = target.trim_end_matches('/');
        let normalized = linkkey(target);
        let path = self
            .note
            .index
            .notes
            .iter()
            .find(|note| {
                linkkey(&note.title) == normalized
                    || linkkey(&note.stem()) == normalized
                    || linkkey(note.path.trim_end_matches(".md")) == normalized
            })
            .map(|note| note.path.clone());
        if let Some(path) = path {
            self.select_note(&path, cx);
        } else {
            self.push_error("Linked note was not found", target.to_string());
        }
    }

    fn note_changed(&mut self, content: &str, fragment: Option<String>, cx: &mut Context<Self>) {
        let Some(path) = self.note.path.clone() else {
            return;
        };
        self.note.saved = false;
        match notes::write(&self.note.root, &path, content) {
            Ok(note) => {
                if let Some(existing) = self
                    .note
                    .index
                    .notes
                    .iter_mut()
                    .find(|existing| existing.path == path)
                {
                    *existing = note.clone();
                }
                self.note.suggestions = fragment
                    .map(|fragment| notes::suggest(&self.note.index, &fragment, 5))
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|candidate| Some(&candidate.path) != self.note.path.as_ref())
                    .collect();
                self.note.saved = true;
                self.update_note_preview(&note, cx);
            }
            Err(error) => self.push_error("Note is not saved", error.to_string()),
        }
        cx.notify();
    }

    fn sync_note_entities(&mut self, cx: &mut Context<Self>) {
        let Some(note) = self.current_note().cloned() else {
            return;
        };
        if let Some(editor) = self.note.editor.clone() {
            if editor.read(cx).text() != note.content {
                editor.update(cx, |editor, cx| editor.set_text(&note.content, cx));
            }
        }
        if let Some(title) = self.note.title.clone() {
            if title.read(cx).text() != note.title {
                title.update(cx, |input, cx| input.set_text(&note.title, cx));
            }
        }
        self.update_note_preview(&note, cx);
    }

    pub(super) fn update_note_preview(&mut self, note: &notes::Note, cx: &mut Context<Self>) {
        if self.note.previewed.as_deref() == Some(note.content.as_str()) {
            return;
        }
        let Some(preview) = self.note.preview.clone() else {
            return;
        };
        let html = preview::html_markdown(&notes::preview_source(note));
        preview.update(cx, |view, cx| view.load_html(html, cx));
        self.note.previewed = Some(note.content.clone());
    }
}

fn completion(editor: &guise::Editor) -> Option<String> {
    let model = editor.model();
    let cursor = model.cursor();
    let line = model.line(cursor.line)?;
    let before: String = line.chars().take(cursor.col).collect();
    notes::completion_fragment(&before)
}

fn linkkey(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
