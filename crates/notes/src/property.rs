//! Frontmatter property editing: set or remove a single YAML key while leaving
//! the rest of the note (body and other keys) untouched. Values are written as
//! plain scalars, which covers the property model the app exposes.

use std::path::Path;

use crate::vault::{read, write};
use crate::{Note, Result};

/// Set (insert or replace) a scalar frontmatter property on a note.
pub fn set_property(root: &Path, relative: &str, name: &str, value: &str) -> Result<Note> {
    let note = read(root, relative)?;
    let content = upsert(&note.content, name.trim(), value.trim());
    write(root, relative, &content)
}

/// Remove a frontmatter property from a note. A no-op if it isn't present.
pub fn remove_property(root: &Path, relative: &str, name: &str) -> Result<Note> {
    let note = read(root, relative)?;
    let content = delete(&note.content, name.trim());
    write(root, relative, &content)
}

fn upsert(content: &str, name: &str, value: &str) -> String {
    let trailing = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    let line = format!("{name}: {value}");
    if let Some(close) = frontmatter_close(&lines) {
        let replaced = lines[1..close]
            .iter_mut()
            .find(|existing| is_key(existing, name))
            .map(|existing| *existing = line.clone())
            .is_some();
        if !replaced {
            lines.insert(close, line);
        }
        return rejoin(lines, trailing);
    }
    let mut out = vec!["---".to_string(), line, "---".to_string(), String::new()];
    out.extend(lines);
    rejoin(out, trailing)
}

fn delete(content: &str, name: &str) -> String {
    let trailing = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    if let Some(close) = frontmatter_close(&lines) {
        if let Some(index) = lines[1..close].iter().position(|line| is_key(line, name)) {
            lines.remove(index + 1);
        }
    }
    rejoin(lines, trailing)
}

/// Index of the closing `---` line when `lines` opens with a frontmatter fence.
fn frontmatter_close(lines: &[String]) -> Option<usize> {
    if lines.first().map(String::as_str) != Some("---") {
        return None;
    }
    lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, line)| line.as_str() == "---")
        .map(|(index, _)| index)
}

fn is_key(line: &str, name: &str) -> bool {
    line.split_once(':')
        .is_some_and(|(key, _)| key.trim() == name)
}

fn rejoin(lines: Vec<String>, trailing: bool) -> String {
    let mut out = lines.join("\n");
    if trailing {
        out.push('\n');
    }
    out
}
