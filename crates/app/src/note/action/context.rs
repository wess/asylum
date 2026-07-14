use std::collections::BTreeSet;
use std::path::PathBuf;

use gpui::{Context, Window};

use crate::state::{now, Root};
use crate::workspace::TabKind;

use super::vault::vaultpath;

impl Root {
    pub fn create_task_from_note(&mut self) {
        let Some(project_id) = self.project_id else {
            return;
        };
        let Some(note) = self.current_note().cloned() else {
            return;
        };
        let prompt = format!(
            "Complete the work described by the attached project note [[{}]]. Treat that note as the source of truth.",
            note.title
        );
        match self.db.create_task(project_id, &note.title, &prompt, now()) {
            Ok(task) => {
                self.task_id = Some(task.id);
                self.selected_run_id = None;
                if let Err(error) =
                    self.db
                        .attach_note_to_task(project_id, &note.path, task.id, now())
                {
                    self.push_error("Task created, but note was not attached", error.to_string());
                }
                self.append_note_reference(
                    project_id,
                    &note.path,
                    &notes::Reference::task(task.id, &task.title),
                );
                self.open_kind(TabKind::Tasks);
            }
            Err(error) => self.push_error("Could not create task", error.to_string()),
        }
    }

    pub fn attach_note_to_selected_run(&mut self) {
        let Some(project_id) = self.project_id else {
            return;
        };
        let Some(run_id) = self.selected_run_id else {
            self.push_error(
                "No run selected",
                "Select a run before attaching this note.",
            );
            return;
        };
        let Some(note) = self.current_note().cloned() else {
            return;
        };
        let Ok(run) = self.db.run(run_id) else {
            return;
        };
        if let Err(error) = self
            .db
            .attach_note_to_run(project_id, &note.path, run_id, now())
        {
            self.push_error("Could not attach note", error.to_string());
            return;
        }
        self.append_note_reference(
            project_id,
            &note.path,
            &notes::Reference::run(run_id, &run.agent),
        );
        self.push_notice(
            crate::run::NoticeTone::Success,
            "Note attached",
            "Its Markdown will be included in the run's next prompt.",
        );
    }

    pub fn send_note_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run_id) = self.selected_run_id else {
            self.push_error(
                "No run selected",
                "Select a run before sending note context.",
            );
            return;
        };
        let Some(editor) = self.note.editor.clone() else {
            return;
        };
        let Some(selection) = editor.read(cx).model().selected_text() else {
            self.push_error(
                "Nothing selected",
                "Select text in the note, then send it again.",
            );
            return;
        };
        let title = self
            .current_note()
            .map(|note| note.title.clone())
            .unwrap_or_else(|| "Project note".to_string());
        let prompt = format!("Context from project note \"{title}\":\n\n{selection}");
        match self.send_followup(run_id, prompt, window, cx) {
            Ok(()) => self.push_notice(
                crate::run::NoticeTone::Success,
                "Context sent",
                "The selected text was sent to the current run.",
            ),
            Err(error) => self.push_error("Could not send context", error),
        }
    }

    /// Markdown attached to a run or its parent task, ready for prompt injection.
    pub fn note_context_for_run(&self, run_id: i64) -> String {
        let Ok(run) = self.db.run(run_id) else {
            return String::new();
        };
        let Ok(task) = self.db.task(run.task_id) else {
            return String::new();
        };
        let Ok(project) = self.db.project(task.project_id) else {
            return String::new();
        };
        let root = self
            .db
            .note_vault(project.id)
            .ok()
            .flatten()
            .map(|vault| PathBuf::from(vault.path))
            .unwrap_or_else(|| vaultpath(project.id, &project.path, store::NoteVaultMode::Private));
        let paths: BTreeSet<String> = self
            .db
            .task_note_paths(task.id)
            .unwrap_or_default()
            .into_iter()
            .chain(self.db.run_note_paths(run_id).unwrap_or_default())
            .collect();
        let notes: Vec<notes::Note> = paths
            .iter()
            .filter_map(|path| notes::read(&root, path).ok())
            .collect();
        if notes.is_empty() {
            return String::new();
        }
        let mut context = String::from("\n\nProject notes attached as authoritative context:\n");
        for note in notes {
            context.push_str(&format!("\n## {}\n\n{}\n", note.title, note.body.trim()));
        }
        context
    }

    /// Copy task-level note links onto a newly-created run and annotate the
    /// Markdown with the generated run link.
    pub fn inherit_task_notes(&mut self, task_id: i64, run_id: i64) {
        let Ok(task) = self.db.task(task_id) else {
            return;
        };
        let Ok(run) = self.db.run(run_id) else {
            return;
        };
        for path in self.db.task_note_paths(task_id).unwrap_or_default() {
            if let Err(error) = self
                .db
                .attach_note_to_run(task.project_id, &path, run_id, now())
            {
                self.push_error("Could not inherit note context", error.to_string());
                continue;
            }
            self.append_note_reference(
                task.project_id,
                &path,
                &notes::Reference::run(run_id, &run.agent),
            );
        }
    }

    pub fn reference_run_notes(&mut self, run_id: i64, reference: &notes::Reference) {
        let Some(project_id) = self
            .db
            .run(run_id)
            .ok()
            .and_then(|run| self.db.task(run.task_id).ok())
            .map(|task| task.project_id)
        else {
            return;
        };
        for path in self.db.run_note_paths(run_id).unwrap_or_default() {
            self.append_note_reference(project_id, &path, reference);
        }
    }

    fn append_note_reference(&mut self, project_id: i64, path: &str, reference: &notes::Reference) {
        let Some(root) = self.note_root_for_project(project_id) else {
            return;
        };
        match notes::append_reference(&root, path, reference) {
            Ok(note) => {
                if self.project_id == Some(project_id) {
                    if let Some(existing) = self
                        .note
                        .index
                        .notes
                        .iter_mut()
                        .find(|existing| existing.path == path)
                    {
                        *existing = note;
                    }
                }
            }
            Err(error) => self.push_error("Could not update attached note", error.to_string()),
        }
    }

    fn note_root_for_project(&self, project_id: i64) -> Option<PathBuf> {
        let project = self.db.project(project_id).ok()?;
        Some(
            self.db
                .note_vault(project_id)
                .ok()
                .flatten()
                .map(|vault| PathBuf::from(vault.path))
                .unwrap_or_else(|| {
                    vaultpath(project_id, &project.path, store::NoteVaultMode::Private)
                }),
        )
    }
}
