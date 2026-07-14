use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::parse::parse;
use crate::{template, Index, Note, Reference, Template};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("note paths must stay inside the vault and end in .md")]
    UnsafePath,
    #[error("note already exists: {0}")]
    Exists(String),
    #[error("note was not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn index(root: &Path) -> Result<Index> {
    fs::create_dir_all(root)?;
    let mut paths = Vec::new();
    collect(root, root, &mut paths)?;
    paths.sort();
    let notes = paths
        .into_iter()
        .filter_map(|path| read(root, &path).ok())
        .collect();
    Ok(Index { notes })
}

pub fn read(root: &Path, relative: &str) -> Result<Note> {
    let path = safe_path(root, relative)?;
    if !path.is_file() {
        return Err(Error::NotFound(relative.to_string()));
    }
    let content = fs::read_to_string(path)?;
    Ok(parse(relative, &content))
}

pub fn write(root: &Path, relative: &str, content: &str) -> Result<Note> {
    let path = safe_path(root, relative)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, content)?;
    Ok(parse(relative, content))
}

pub fn create(root: &Path, title: &str, kind: Template, created: i64) -> Result<Note> {
    fs::create_dir_all(root)?;
    let title = clean_title(title);
    let base = slug(&title);
    let mut suffix = 1;
    let relative = loop {
        let candidate = if suffix == 1 {
            format!("{base}.md")
        } else {
            format!("{base}{suffix}.md")
        };
        if !safe_path(root, &candidate)?.exists() {
            break candidate;
        }
        suffix += 1;
    };
    write(root, &relative, &template(kind, &title, created))
}

pub fn delete(root: &Path, relative: &str) -> Result<()> {
    let path = safe_path(root, relative)?;
    if !path.exists() {
        return Err(Error::NotFound(relative.to_string()));
    }
    fs::remove_file(path)?;
    Ok(())
}

/// Rename a note and update incoming wiki links across the vault.
pub fn rename(root: &Path, relative: &str, title: &str) -> Result<Note> {
    let old = read(root, relative)?;
    let title = clean_title(title);
    let parent = Path::new(relative)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let next_relative = parent.join(format!("{}.md", slug(&title)));
    let next_relative = slash(&next_relative);
    let old_path = safe_path(root, relative)?;
    let next_path = safe_path(root, &next_relative)?;
    if old_path != next_path && next_path.exists() {
        return Err(Error::Exists(next_relative));
    }
    let content = retitle(&old.content, &title);
    fs::write(&old_path, content)?;
    if old_path != next_path {
        fs::rename(&old_path, &next_path)?;
    }

    let aliases = BTreeSet::from([
        old.title.to_lowercase(),
        old.stem().to_lowercase(),
        relative.trim_end_matches(".md").to_lowercase(),
    ]);
    let all = index(root)?;
    for note in all.notes {
        if note.path == next_relative {
            continue;
        }
        let rewritten = relink(&note.content, &aliases, &title);
        if rewritten != note.content {
            write(root, &note.path, &rewritten)?;
        }
    }
    read(root, &next_relative)
}

pub fn append_reference(root: &Path, relative: &str, reference: &Reference) -> Result<Note> {
    let note = read(root, relative)?;
    let line = reference.markdown();
    if note.content.lines().any(|existing| existing == line) {
        return Ok(note);
    }
    let mut content = note.content.trim_end().to_string();
    if !content.contains("<!-- asylum:links -->") {
        content.push_str("\n\n## Asylum\n\n<!-- asylum:links -->");
    }
    content.push('\n');
    content.push_str(&line);
    content.push('\n');
    write(root, relative, &content)
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            if let Ok(relative) = path.strip_prefix(root) {
                out.push(slash(relative));
            }
        }
    }
    Ok(())
}

fn safe_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative.extension().and_then(|ext| ext.to_str()) != Some("md")
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(Error::UnsafePath);
    }
    Ok(root.join(relative))
}

fn clean_title(title: &str) -> String {
    let title = title.trim();
    if title.is_empty() {
        "Untitled".to_string()
    } else {
        title.chars().take(120).collect()
    }
}

fn slug(title: &str) -> String {
    let slug: String = title
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect();
    if slug.is_empty() {
        "note".to_string()
    } else {
        slug
    }
}

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn retitle(content: &str, title: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    let in_frontmatter = lines.first().is_some_and(|line| line == "---");
    let mut frontmatter_end = None;
    for (index, line) in lines.iter_mut().enumerate().skip(1) {
        if in_frontmatter && line == "---" {
            frontmatter_end = Some(index);
            break;
        }
        if in_frontmatter && line.trim_start().starts_with("title:") {
            *line = format!("title: {title}");
        }
    }
    if let Some(line) = lines.iter_mut().find(|line| line.starts_with("# ")) {
        *line = format!("# {title}");
    } else {
        let at = frontmatter_end.map_or(0, |index| index + 1);
        lines.insert(at, format!("# {title}"));
    }
    let mut result = lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn relink(content: &str, aliases: &BTreeSet<String>, title: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let raw = &after[..end];
        let mut alias = raw.splitn(2, '|');
        let target = alias.next().unwrap_or_default();
        let (base, heading) = target.split_once('#').unwrap_or((target, ""));
        if aliases.contains(&base.trim().to_lowercase()) {
            out.push_str("[[");
            out.push_str(title);
            if !heading.is_empty() {
                out.push('#');
                out.push_str(heading);
            }
            if let Some(alias) = alias.next() {
                out.push('|');
                out.push_str(alias);
            }
            out.push_str("]]");
        } else {
            out.push_str("[[");
            out.push_str(raw);
            out.push_str("]]");
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}
