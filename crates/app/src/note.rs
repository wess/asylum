mod action;
mod view;

use gpui::Entity;
use guise::{Editor, TextInput, WebView};

pub use view::surface;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Edit,
    Preview,
    Split,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Files,
    Write,
    Links,
}

pub struct State {
    pub project_id: Option<i64>,
    pub root: std::path::PathBuf,
    pub mode: store::NoteVaultMode,
    pub index: notes::Index,
    pub path: Option<String>,
    pub editor: Option<Entity<Editor>>,
    pub preview: Option<Entity<WebView>>,
    pub search: Option<Entity<TextInput>>,
    pub title: Option<Entity<TextInput>>,
    pub query: String,
    pub view: Mode,
    pub panel: Panel,
    pub files_open: bool,
    pub details_open: bool,
    pub templates_open: bool,
    pub template: notes::Template,
    pub suggestions: Vec<notes::Note>,
    pub saved: bool,
    pub previewed: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            project_id: None,
            root: std::path::PathBuf::new(),
            mode: store::NoteVaultMode::Private,
            index: notes::Index::default(),
            path: None,
            editor: None,
            preview: None,
            search: None,
            title: None,
            query: String::new(),
            view: Mode::Edit,
            panel: Panel::Write,
            files_open: true,
            details_open: true,
            templates_open: false,
            template: notes::Template::Blank,
            suggestions: Vec::new(),
            saved: true,
            previewed: None,
        }
    }
}
