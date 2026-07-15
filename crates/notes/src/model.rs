use std::path::Path;

/// One frontmatter property, retained as a displayable YAML value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub name: String,
    pub value: String,
}

/// A wiki-style `[[target|alias]]` link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub target: String,
    pub alias: Option<String>,
}

/// A parsed Markdown note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// Slash-separated path relative to the vault.
    pub path: String,
    pub title: String,
    /// Original Markdown, including frontmatter.
    pub content: String,
    /// Markdown after frontmatter, suitable for task context and preview.
    pub body: String,
    pub properties: Vec<Property>,
    pub tags: Vec<String>,
    pub links: Vec<Link>,
}

impl Note {
    pub fn stem(&self) -> String {
        Path::new(&self.path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(&self.title)
            .to_string()
    }
}

/// A complete in-memory vault index used for links, backlinks, and completion.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Index {
    pub notes: Vec<Note>,
}

impl Index {
    pub fn note(&self, path: &str) -> Option<&Note> {
        self.notes.iter().find(|note| note.path == path)
    }

    pub fn backlinks(&self, note: &Note) -> Vec<&Note> {
        let title = normalize(&note.title);
        let stem = normalize(&note.stem());
        let path = normalize(note.path.trim_end_matches(".md"));
        self.notes
            .iter()
            .filter(|candidate| candidate.path != note.path)
            .filter(|candidate| {
                candidate.links.iter().any(|link| {
                    let target = normalize(link.target.split('#').next().unwrap_or_default());
                    target == title || target == stem || target == path
                })
            })
            .collect()
    }

    pub fn outgoing<'a>(&'a self, note: &Note) -> Vec<&'a Note> {
        note.links
            .iter()
            .filter_map(|link| self.resolve(&link.target))
            .collect()
    }

    pub fn resolve(&self, target: &str) -> Option<&Note> {
        let target = normalize(target.split('#').next().unwrap_or_default());
        self.notes.iter().find(|note| {
            normalize(&note.title) == target
                || normalize(&note.stem()) == target
                || normalize(note.path.trim_end_matches(".md")) == target
        })
    }
}

fn normalize(value: &str) -> String {
    value.trim().replace('\\', "/").to_lowercase()
}

/// Built-in note structures for common engineering work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    Blank,
    Task,
    Decision,
    Investigation,
    Retrospective,
}

impl Template {
    pub const ALL: [Template; 5] = [
        Template::Blank,
        Template::Task,
        Template::Decision,
        Template::Investigation,
        Template::Retrospective,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Blank => "Blank note",
            Self::Task => "Task brief",
            Self::Decision => "Decision",
            Self::Investigation => "Investigation",
            Self::Retrospective => "Retrospective",
        }
    }
}

/// A durable Markdown link from project knowledge into the ADE lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub kind: ReferenceKind,
    pub target: String,
    pub label: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Task,
    Run,
    Checks,
    PullRequest,
}

impl Reference {
    pub fn task(id: i64, title: &str) -> Self {
        Self {
            kind: ReferenceKind::Task,
            target: format!("asylum://task/{id}"),
            label: format!("Task #{id}: {title}"),
            detail: None,
        }
    }

    pub fn run(id: i64, agent: &str) -> Self {
        Self {
            kind: ReferenceKind::Run,
            target: format!("asylum://run/{id}"),
            label: format!("Run #{id}: {agent}"),
            detail: None,
        }
    }

    pub fn checks(id: i64, summary: &str) -> Self {
        Self {
            kind: ReferenceKind::Checks,
            target: format!("asylum://run/{id}/checks"),
            label: format!("Checks for run #{id}"),
            detail: Some(summary.to_string()),
        }
    }

    pub fn pullrequest(url: &str) -> Self {
        Self {
            kind: ReferenceKind::PullRequest,
            target: url.to_string(),
            label: "Pull request".to_string(),
            detail: None,
        }
    }

    pub fn markdown(&self) -> String {
        let kind = match self.kind {
            ReferenceKind::Task => "Task",
            ReferenceKind::Run => "Run",
            ReferenceKind::Checks => "Checks",
            ReferenceKind::PullRequest => "PR",
        };
        match &self.detail {
            Some(detail) => format!("- {kind}: [{}]({}) - {detail}", self.label, self.target),
            None => format!("- {kind}: [{}]({})", self.label, self.target),
        }
    }
}
