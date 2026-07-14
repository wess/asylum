use crate::{Reference, Template};

#[test]
fn parses_obsidian_links_properties_tags_and_backlinks() {
    let source = "---\ntitle: Project Brief\ntags: [asylum, planning]\nowner: wess\n---\n# Project Brief\n\nSee [[Decision Log|decisions]]. #active";
    let note = crate::parse("brief.md", source);
    assert_eq!(note.title, "Project Brief");
    assert!(note.tags.contains(&"asylum".to_string()));
    assert!(note.tags.contains(&"active".to_string()));
    assert_eq!(note.links[0].target, "Decision Log");
    assert_eq!(note.links[0].alias.as_deref(), Some("decisions"));
    assert!(note
        .properties
        .iter()
        .any(|property| property.name == "owner"));

    let decision = crate::parse("decisionlog.md", "# Decision Log\n");
    let index = crate::Index {
        notes: vec![note.clone(), decision.clone()],
    };
    assert_eq!(index.backlinks(&decision)[0].path, note.path);
    assert_eq!(index.outgoing(&note)[0].path, decision.path);
}

#[test]
fn vault_crud_renames_incoming_links_and_rejects_escape() {
    let dir = tempfile::tempdir().unwrap();
    let first = crate::create(dir.path(), "Decision Log", Template::Decision, 10).unwrap();
    let second = crate::write(
        dir.path(),
        "brief.md",
        "# Brief\n\nSee [[Decision Log|the decision]].",
    )
    .unwrap();
    let renamed = crate::rename(dir.path(), &first.path, "Architecture Decision").unwrap();
    assert_eq!(renamed.path, "architecturedecision.md");
    let second = crate::read(dir.path(), &second.path).unwrap();
    assert!(second
        .content
        .contains("[[Architecture Decision|the decision]]"));
    assert!(crate::write(dir.path(), "../escape.md", "no").is_err());
    crate::delete(dir.path(), &renamed.path).unwrap();
    assert!(crate::read(dir.path(), &renamed.path).is_err());
}

#[test]
fn templates_search_completion_and_references_are_operational() {
    let dir = tempfile::tempdir().unwrap();
    let note = crate::create(dir.path(), "Cache failure", Template::Investigation, 22).unwrap();
    let source = format!("{}\nEvidence says the cache key is stale.", note.content);
    crate::write(dir.path(), &note.path, &source).unwrap();
    crate::append_reference(dir.path(), &note.path, &Reference::task(7, "Fix cache")).unwrap();
    let index = crate::index(dir.path()).unwrap();
    let hits = crate::search(&index, "stale");
    assert_eq!(hits[0].path, note.path);
    assert_eq!(
        crate::completion_fragment("compare [[Cac").as_deref(),
        Some("Cac")
    );
    assert_eq!(crate::suggest(&index, "cac", 5)[0].title, "Cache failure");
    assert!(index.notes[0].content.contains("asylum://task/7"));
    assert!(crate::preview_source(&index.notes[0]).contains("asylum://task/7"));
}
