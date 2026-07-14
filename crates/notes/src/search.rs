use crate::{Index, Note};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub score: usize,
}

pub fn search(index: &Index, query: &str) -> Vec<Hit> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return index
            .notes
            .iter()
            .take(100)
            .map(|note| hit(note, 0, first_line(&note.body)))
            .collect();
    }
    let mut hits: Vec<Hit> = index
        .notes
        .iter()
        .filter_map(|note| {
            let title = note.title.to_lowercase();
            let path = note.path.to_lowercase();
            let body = note.body.to_lowercase();
            if title == query {
                Some(hit(note, 100, first_line(&note.body)))
            } else if title.starts_with(&query) {
                Some(hit(note, 80, first_line(&note.body)))
            } else if title.contains(&query) {
                Some(hit(note, 60, first_line(&note.body)))
            } else if path.contains(&query) {
                Some(hit(note, 40, first_line(&note.body)))
            } else {
                body.find(&query).map(|offset| {
                    let original = note.body.get(offset..).unwrap_or(&note.body);
                    hit(note, 20, first_line(original))
                })
            }
        })
        .collect();
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
    hits
}

pub fn suggest(index: &Index, fragment: &str, limit: usize) -> Vec<Note> {
    let fragment = fragment.trim().to_lowercase();
    let mut notes: Vec<(usize, &Note)> = index
        .notes
        .iter()
        .filter_map(|note| {
            let title = note.title.to_lowercase();
            let score = if fragment.is_empty() {
                1
            } else if title.starts_with(&fragment) {
                3
            } else if title.contains(&fragment) {
                2
            } else {
                return None;
            };
            Some((score, note))
        })
        .collect();
    notes
        .sort_by(|(ascore, a), (bscore, b)| bscore.cmp(ascore).then_with(|| a.title.cmp(&b.title)));
    notes
        .into_iter()
        .take(limit)
        .map(|(_, note)| note.clone())
        .collect()
}

fn hit(note: &Note, score: usize, snippet: &str) -> Hit {
    Hit {
        path: note.path.clone(),
        title: note.title.clone(),
        snippet: snippet.chars().take(180).collect(),
        score,
    }
}

fn first_line(value: &str) -> &str {
    value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .unwrap_or_default()
}
