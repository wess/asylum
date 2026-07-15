use std::collections::BTreeSet;

use serde_yaml_ng::Value;

use crate::{Index, Link, Note, Property};

/// Parse one Markdown file without changing its source.
pub fn parse(path: &str, content: &str) -> Note {
    let (frontmatter, body) = split_frontmatter(content);
    let properties = parse_properties(frontmatter);
    let title = properties
        .iter()
        .find(|property| property.name.eq_ignore_ascii_case("title"))
        .map(|property| unquote(&property.value))
        .filter(|title| !title.is_empty())
        .or_else(|| heading(body))
        .unwrap_or_else(|| {
            std::path::Path::new(path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("Untitled")
                .to_string()
        });
    let links = wiki_links(body);
    let mut tags = property_tags(frontmatter);
    tags.extend(inline_tags(body));
    let tags = tags.into_iter().collect();
    Note {
        path: path.to_string(),
        title,
        content: content.to_string(),
        body: body.to_string(),
        properties,
        tags,
        links,
    }
}

/// Return the unfinished target after the last `[[` before the caret.
pub fn completion_fragment(line_before_caret: &str) -> Option<String> {
    let start = line_before_caret.rfind("[[")?;
    let fragment = &line_before_caret[start + 2..];
    (!fragment.contains("]]")).then(|| fragment.split('|').next().unwrap_or_default().to_string())
}

/// Markdown for preview: frontmatter is rendered in the property pane and wiki
/// links become ordinary links while keeping their original spelling useful.
/// Without an index, embeds (`![[note]]`) degrade to plain links rather than
/// broken images.
pub fn preview_source(note: &Note) -> String {
    rewrite(&note.body, None, 0)
}

/// Markdown for preview, resolving embeds against a vault index. `![[note]]`
/// and `![[note#heading]]` inline the target note (or section) as a quoted
/// block; unresolved embeds render a short notice instead of broken markup.
pub fn preview_source_in(index: &Index, note: &Note) -> String {
    rewrite(&note.body, Some(index), 0)
}

pub(crate) fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let normalized = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"));
    let Some(rest) = normalized else {
        return (None, content);
    };
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let clean = line.trim_end_matches(['\r', '\n']);
        if clean == "---" {
            let frontmatter = &rest[..offset];
            let body = &rest[offset + line.len()..];
            return (Some(frontmatter), body);
        }
        offset += line.len();
    }
    (None, content)
}

fn parse_properties(source: Option<&str>) -> Vec<Property> {
    let Some(source) = source else {
        return Vec::new();
    };
    let Ok(Value::Mapping(map)) = serde_yaml_ng::from_str::<Value>(source) else {
        return Vec::new();
    };
    map.into_iter()
        .filter_map(|(key, value)| {
            let Value::String(name) = key else {
                return None;
            };
            Some(Property {
                name,
                value: yaml_value(&value),
            })
        })
        .collect()
}

fn yaml_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Sequence(values) => values.iter().map(yaml_value).collect::<Vec<_>>().join(", "),
        Value::Mapping(_) | Value::Tagged(_) => serde_yaml_ng::to_string(value)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

fn property_tags(source: Option<&str>) -> BTreeSet<String> {
    let Some(source) = source else {
        return BTreeSet::new();
    };
    let Ok(Value::Mapping(map)) = serde_yaml_ng::from_str::<Value>(source) else {
        return BTreeSet::new();
    };
    let mut tags = BTreeSet::new();
    for (key, value) in map {
        if !matches!(key, Value::String(ref key) if key.eq_ignore_ascii_case("tags")) {
            continue;
        }
        match value {
            Value::Sequence(values) => {
                for value in values {
                    add_tag(&mut tags, &yaml_value(&value));
                }
            }
            value => {
                for tag in yaml_value(&value).split([',', ' ']) {
                    add_tag(&mut tags, tag);
                }
            }
        }
    }
    tags
}

fn inline_tags(body: &str) -> BTreeSet<String> {
    let mut tags = BTreeSet::new();
    let mut fenced = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        for (index, ch) in chars.iter().enumerate() {
            if *ch != '#' || index > 0 && !chars[index - 1].is_whitespace() {
                continue;
            }
            let tag: String = chars[index + 1..]
                .iter()
                .take_while(|ch| ch.is_alphanumeric() || matches!(ch, '-' | '_' | '/'))
                .collect();
            add_tag(&mut tags, &tag);
        }
    }
    tags
}

fn add_tag(tags: &mut BTreeSet<String>, value: &str) {
    let value = value.trim().trim_start_matches('#');
    if !value.is_empty() {
        tags.insert(value.to_string());
    }
}

fn heading(body: &str) -> Option<String> {
    body.lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map(str::to_string)
}

fn wiki_links(body: &str) -> Vec<Link> {
    let mut links = Vec::new();
    let mut rest = body;
    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("]]") else {
            break;
        };
        let raw = &rest[..end];
        rest = &rest[end + 2..];
        let mut parts = raw.splitn(2, '|');
        let target = parts.next().unwrap_or_default().trim();
        if target.is_empty() {
            continue;
        }
        let alias = parts
            .next()
            .map(str::trim)
            .filter(|alias| !alias.is_empty())
            .map(str::to_string);
        links.push(Link {
            target: target.to_string(),
            alias,
        });
    }
    links
}

/// Rewrite `[[link]]` and `![[embed]]` wiki syntax into preview-ready Markdown.
/// `depth` bounds embed transclusion so a note that embeds itself (directly or
/// in a cycle) cannot recurse forever.
fn rewrite(body: &str, index: Option<&Index>, depth: u8) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(pos) = rest.find("[[") {
        let is_embed = pos > 0 && rest.as_bytes()[pos - 1] == b'!';
        let prefix_end = if is_embed { pos - 1 } else { pos };
        out.push_str(&rest[..prefix_end]);
        let after = &rest[pos + 2..];
        let Some(end) = after.find("]]") else {
            out.push_str(&rest[prefix_end..]);
            return out;
        };
        let (target, alias) = split_alias(&after[..end]);
        rest = &after[end + 2..];
        if is_embed {
            out.push_str(&embed(index, target, alias, depth));
        } else {
            out.push_str(&link_markdown(target, alias));
        }
    }
    out.push_str(rest);
    out
}

/// Split a wiki-link body into its target and optional display alias.
fn split_alias(raw: &str) -> (&str, Option<&str>) {
    let mut parts = raw.splitn(2, '|');
    let target = parts.next().unwrap_or_default().trim();
    let alias = parts
        .next()
        .map(str::trim)
        .filter(|alias| !alias.is_empty());
    (target, alias)
}

/// Render a `[[target|alias]]` link as a Markdown link into the note scheme.
/// The `#heading` fragment is kept in the display text but dropped from the
/// slug, which addresses the note as a whole.
fn link_markdown(target: &str, alias: Option<&str>) -> String {
    let display = alias.unwrap_or(target);
    let name = target.split('#').next().unwrap_or(target).trim();
    format!("[{display}](asylum://note/{})", slug(name))
}

/// Render an `![[target]]` embed. Note embeds are transcluded as a quoted block
/// when an index is available and the depth budget allows; image embeds become
/// real images; everything else degrades to a plain link.
fn embed(index: Option<&Index>, target: &str, alias: Option<&str>, depth: u8) -> String {
    let name = target.split('#').next().unwrap_or(target).trim();
    let heading = target.split_once('#').map(|(_, tail)| tail.trim());
    if is_image(name) {
        let alt = alias.unwrap_or(name);
        return format!("![{alt}]({target})");
    }
    if let Some(index) = index.filter(|_| depth < 1) {
        if let Some(note) = index.resolve(name) {
            let content = heading
                .and_then(|heading| extract_section(&note.body, heading))
                .unwrap_or_else(|| note.body.clone());
            let inner = rewrite(content.trim(), Some(index), depth + 1);
            let title = match heading {
                Some(heading) => format!("{} › {heading}", note.title),
                None => note.title.clone(),
            };
            let quoted = inner
                .lines()
                .map(|line| {
                    if line.is_empty() {
                        ">".to_string()
                    } else {
                        format!("> {line}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            return format!("\n> **{title}**\n>\n{quoted}\n\n");
        }
        return format!("\n> _Embedded note \"{name}\" was not found._\n\n");
    }
    link_markdown(target, alias)
}

/// Content of the section under `heading`: the lines after the matching heading
/// up to the next heading of the same or shallower level.
fn extract_section(body: &str, heading: &str) -> Option<String> {
    let wanted = heading.trim().to_lowercase();
    let mut lines = body.lines();
    let mut level = 0;
    for line in lines.by_ref() {
        if let Some((depth, text)) = heading_parts(line) {
            if text.to_lowercase() == wanted {
                level = depth;
                break;
            }
        }
    }
    if level == 0 {
        return None;
    }
    let mut collected = Vec::new();
    for line in lines {
        if let Some((depth, _)) = heading_parts(line) {
            if depth <= level {
                break;
            }
        }
        collected.push(line);
    }
    Some(collected.join("\n"))
}

/// Parse a Markdown ATX heading into its level and trimmed text.
fn heading_parts(line: &str) -> Option<(usize, &str)> {
    let hashes = line.chars().take_while(|ch| *ch == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &line[hashes..];
    rest.strip_prefix(' ').map(|text| (hashes, text.trim()))
}

fn is_image(target: &str) -> bool {
    let lower = target.to_lowercase();
    [
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".bmp", ".ico", ".avif",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

fn slug(name: &str) -> String {
    name.chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { '-' })
        .collect()
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches(['\'', '"']).to_string()
}
