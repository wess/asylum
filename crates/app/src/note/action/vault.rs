use std::path::{Path, PathBuf};

use gpui::Context;

use crate::state::{now, Root};

impl Root {
    pub fn current_note(&self) -> Option<&notes::Note> {
        self.note
            .path
            .as_deref()
            .and_then(|path| self.note.index.note(path))
    }

    pub fn filtered_notes(&self) -> Vec<notes::Note> {
        notes::search(&self.note.index, &self.note.query)
            .into_iter()
            .filter_map(|hit| self.note.index.note(&hit.path).cloned())
            .filter(|note| match &self.note.tag_filter {
                Some(tag) => note.tags.iter().any(|t| t == tag),
                None => true,
            })
            .collect()
    }

    /// Filter the note list to a tag (clicked from the details pane).
    pub fn set_note_tag_filter(&mut self, tag: String, cx: &mut Context<Self>) {
        self.note.tag_filter = Some(tag);
        cx.notify();
    }

    /// Clear the active tag filter.
    pub fn clear_note_tag_filter(&mut self, cx: &mut Context<Self>) {
        self.note.tag_filter = None;
        cx.notify();
    }

    /// Add a frontmatter property from the `name: value` input.
    pub fn add_note_property(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.note.path.clone() else {
            return;
        };
        let Some(input) = self.note.property_input.clone() else {
            return;
        };
        let raw = input.read(cx).text();
        let Some((name, value)) = raw.split_once(':') else {
            self.push_error("Property needs a value", "Write it as name: value.");
            return;
        };
        let (name, value) = (name.trim(), value.trim());
        if name.is_empty() {
            return;
        }
        match notes::set_property(&self.note.root, &path, name, value) {
            Ok(_) => {
                input.update(cx, |input, cx| input.set_text("", cx));
                self.refresh_current_note(cx);
            }
            Err(error) => self.push_error("Could not set property", error.to_string()),
        }
    }

    /// Remove a frontmatter property by name.
    pub fn remove_note_property(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some(path) = self.note.path.clone() else {
            return;
        };
        match notes::remove_property(&self.note.root, &path, name) {
            Ok(_) => self.refresh_current_note(cx),
            Err(error) => self.push_error("Could not remove property", error.to_string()),
        }
    }

    /// Create a note from a user-authored template.
    pub fn create_note_from_template(&mut self, body: String, cx: &mut Context<Self>) {
        match notes::create_from_template(&self.note.root, &body, "Untitled", now()) {
            Ok(note) => self.refresh_note_index(Some(note.path), cx),
            Err(error) => self.push_error("Could not create note", error.to_string()),
        }
    }

    /// Reload the on-disk copy of the current note after an out-of-editor edit,
    /// refreshing the editor, preview, and details pane.
    fn refresh_current_note(&mut self, cx: &mut Context<Self>) {
        let selected = self.note.path.clone();
        self.refresh_note_index(selected, cx);
    }

    pub fn select_note(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(note) = self.note.index.note(path).cloned() else {
            self.push_error("Note unavailable", "Refresh the vault and select it again.");
            return;
        };
        self.note.path = Some(note.path.clone());
        self.note.suggestions.clear();
        self.note.saved = true;
        if let Some(editor) = self.note.editor.clone() {
            editor.update(cx, |editor, cx| editor.set_text(&note.content, cx));
        }
        if let Some(title) = self.note.title.clone() {
            title.update(cx, |input, cx| input.set_text(&note.title, cx));
        }
        self.update_note_preview(&note, cx);
        cx.notify();
    }

    pub fn create_note(&mut self, kind: notes::Template, cx: &mut Context<Self>) {
        let title = match kind {
            notes::Template::Blank => "Untitled",
            notes::Template::Task => "Task brief",
            notes::Template::Decision => "Decision",
            notes::Template::Investigation => "Investigation",
            notes::Template::Retrospective => "Retrospective",
        };
        match notes::create(&self.note.root, title, kind, now()) {
            Ok(note) => {
                self.note.template = kind;
                self.refresh_note_index(Some(note.path), cx);
            }
            Err(error) => self.push_error("Could not create note", error.to_string()),
        }
    }

    pub fn rename_current_note(&mut self, cx: &mut Context<Self>) {
        let Some(old_path) = self.note.path.clone() else {
            return;
        };
        let Some(input) = self.note.title.clone() else {
            return;
        };
        let title = input.read(cx).text();
        match notes::rename(&self.note.root, &old_path, &title) {
            Ok(note) => {
                if let Some(project_id) = self.project_id {
                    if let Err(error) = self
                        .db
                        .rename_note_attachments(project_id, &old_path, &note.path)
                    {
                        self.push_error(
                            "Note renamed, but links were not saved",
                            error.to_string(),
                        );
                    }
                }
                self.refresh_note_index(Some(note.path), cx);
            }
            Err(error) => self.push_error("Could not rename note", error.to_string()),
        }
    }

    pub fn request_delete_note(&mut self) {
        if let Some(path) = self.note.path.clone() {
            self.confirm = Some(crate::run::ConfirmAction::DeleteNote(path));
        }
    }

    pub fn delete_note(&mut self, path: &str, cx: &mut Context<Self>) {
        match notes::delete(&self.note.root, path) {
            Ok(()) => {
                if let Some(project_id) = self.project_id {
                    let _ = self.db.delete_note_attachments(project_id, path);
                }
                self.note.path = None;
                self.note.index = notes::index(&self.note.root).unwrap_or_default();
                let next = self.note.index.notes.first().map(|note| note.path.clone());
                if let Some(next) = next {
                    self.select_note(&next, cx);
                } else {
                    if let Some(editor) = self.note.editor.clone() {
                        editor.update(cx, |editor, cx| editor.set_text("", cx));
                    }
                    if let Some(title) = self.note.title.clone() {
                        title.update(cx, |input, cx| input.set_text("", cx));
                    }
                }
            }
            Err(error) => self.push_error("Could not delete note", error.to_string()),
        }
    }

    pub fn refresh_notes(&mut self, cx: &mut Context<Self>) {
        let selected = self.note.path.clone();
        self.refresh_note_index(selected, cx);
    }

    /// Switch storage without deleting the old vault. Missing notes are copied
    /// into the target; conflicting target files win and are reported.
    pub fn set_note_vault_mode(&mut self, mode: store::NoteVaultMode, cx: &mut Context<Self>) {
        if self.note.mode == mode {
            return;
        }
        let Some(project_id) = self.project_id else {
            return;
        };
        let Ok(project) = self.db.project(project_id) else {
            return;
        };
        let target = vaultpath(project_id, &project.path, mode);
        let mut conflicts = 0;
        for note in &self.note.index.notes {
            if notes::read(&target, &note.path).is_ok() {
                conflicts += 1;
            } else if let Err(error) = notes::write(&target, &note.path, &note.content) {
                self.push_error("Could not copy note", error.to_string());
                return;
            }
        }
        let path = target.to_string_lossy().into_owned();
        match self.db.set_note_vault(project_id, mode, &path) {
            Ok(_) => {
                let selected = self.note.path.clone();
                self.note.root = target;
                self.note.mode = mode;
                self.refresh_note_index(selected, cx);
                if conflicts > 0 {
                    self.push_notice(
                        crate::run::NoticeTone::Warning,
                        "Vault switched with existing files",
                        format!("Kept {conflicts} note(s) already present in the target vault."),
                    );
                }
            }
            Err(error) => self.push_error("Could not switch vault", error.to_string()),
        }
    }

    pub(super) fn load_project_notes(&mut self, cx: &mut Context<Self>) {
        let Some(project_id) = self.project_id else {
            return;
        };
        let Ok(project) = self.db.project(project_id) else {
            return;
        };
        let vault = self.db.note_vault(project_id).ok().flatten();
        let mode = vault
            .as_ref()
            .map(|vault| vault.mode)
            .unwrap_or(store::NoteVaultMode::Private);
        let root = vault
            .map(|vault| PathBuf::from(vault.path))
            .unwrap_or_else(|| vaultpath(project_id, &project.path, mode));
        if self.db.note_vault(project_id).ok().flatten().is_none() {
            let _ = self
                .db
                .set_note_vault(project_id, mode, &root.to_string_lossy());
        }
        self.note.project_id = Some(project_id);
        self.note.root = root;
        self.note.mode = mode;
        self.refresh_note_index(None, cx);
    }

    fn refresh_note_index(&mut self, selected: Option<String>, cx: &mut Context<Self>) {
        match notes::index(&self.note.root) {
            Ok(index) => {
                self.note.index = index;
                let selected = selected
                    .filter(|path| self.note.index.note(path).is_some())
                    .or_else(|| self.note.index.notes.first().map(|note| note.path.clone()));
                self.note.path = selected.clone();
                if let Some(path) = selected {
                    self.select_note(&path, cx);
                } else {
                    if let Some(editor) = self.note.editor.clone() {
                        editor.update(cx, |editor, cx| editor.set_text("", cx));
                    }
                    if let Some(title) = self.note.title.clone() {
                        title.update(cx, |input, cx| input.set_text("", cx));
                    }
                }
            }
            Err(error) => self.push_error("Could not read notes", error.to_string()),
        }
    }
}

pub(super) fn vaultpath(
    project_id: i64,
    project_path: &str,
    mode: store::NoteVaultMode,
) -> PathBuf {
    match mode {
        store::NoteVaultMode::Private => Root::db_path()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("notes")
            .join(format!("project{project_id}")),
        store::NoteVaultMode::Repository => Path::new(project_path).join("notes"),
    }
}
