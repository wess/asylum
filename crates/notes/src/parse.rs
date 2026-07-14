use std::collections::BTreeSet;

use serde_yaml_ng::Value;

use crate::{Link, Note, Property};

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
/// links become ordinary links while keeping their Obsidian spelling useful.
pub fn preview_source(note: &Note) -> String {
    rewrite_wiki_links(&note.body)
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

fn rewrite_wiki_links(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let raw = &after[..end];
        let mut parts = raw.splitn(2, '|');
        let target = parts.next().unwrap_or_default().trim();
        let alias = parts.next().map(str::trim).unwrap_or(target);
        let slug = target
            .chars()
            .map(|ch| if ch.is_alphanumeric() { ch } else { '-' })
            .collect::<String>();
        out.push_str(&format!("[{alias}](asylum://note/{slug})"));
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches(['\'', '"']).to_string()
}
