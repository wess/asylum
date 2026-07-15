use crate::{Reference, Template};

#[test]
fn parses_wiki_links_properties_tags_and_backlinks() {
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

#[test]
fn embeds_transclude_notes_and_sections_without_broken_images() {
    let short = crate::parse("short.md", "# Short\n\nJust a line.");
    let policy = crate::parse(
        "policy.md",
        "# Cache policy\n\n## Details\n\nEvict on write.\n\n## Other\n\nUnrelated.",
    );
    let host = crate::parse(
        "brief.md",
        "Intro.\n\n![[Short]]\n\n![[Cache policy#Details]]\n\n![[missing]]\n\n![[diagram.png]]",
    );
    let index = crate::Index {
        notes: vec![short, policy, host.clone()],
    };
    let rendered = crate::preview_source_in(&index, &host);
    // Whole-note embed is transcluded as a quoted block, not a broken image.
    assert!(rendered.contains("> **Short**"));
    assert!(rendered.contains("> Just a line."));
    assert!(!rendered.contains("![Short]"));
    // Section embed pulls only the requested heading's content, not its sibling.
    assert!(rendered.contains("> **Cache policy › Details**"));
    assert!(rendered.contains("> Evict on write."));
    assert!(!rendered.contains("Unrelated."));
    // Unresolved note embeds degrade to a notice; image embeds stay images.
    assert!(rendered.contains("was not found"));
    assert!(rendered.contains("![diagram.png](diagram.png)"));
}

#[test]
fn properties_can_be_set_and_removed_without_touching_the_body() {
    let dir = tempfile::tempdir().unwrap();
    crate::write(
        dir.path(),
        "note.md",
        "---\ntitle: Note\nstatus: draft\n---\n# Note\n\nBody stays.",
    )
    .unwrap();

    // Replace an existing key, add a new one.
    crate::set_property(dir.path(), "note.md", "status", "shipped").unwrap();
    let note = crate::set_property(dir.path(), "note.md", "owner", "wess").unwrap();
    assert!(note.content.contains("status: shipped"));
    assert!(note.content.contains("owner: wess"));
    assert!(note.content.contains("Body stays."));
    assert_eq!(
        note.properties
            .iter()
            .find(|p| p.name == "owner")
            .unwrap()
            .value,
        "wess"
    );

    // Remove a key.
    let note = crate::remove_property(dir.path(), "note.md", "status").unwrap();
    assert!(!note.content.contains("status:"));
    assert!(note.content.contains("owner: wess"));

    // Setting a property on a note without frontmatter creates the block.
    crate::write(dir.path(), "plain.md", "# Plain\n\nNo frontmatter here.").unwrap();
    let note = crate::set_property(dir.path(), "plain.md", "type", "memo").unwrap();
    assert!(note.content.starts_with("---\ntype: memo\n---"));
    assert!(note.content.contains("No frontmatter here."));
}

#[test]
fn user_templates_render_variables_and_created_is_a_date() {
    let dir = tempfile::tempdir().unwrap();
    crate::save_user_template(dir.path(), "Standup", "# {{title}}\n\nDate: {{date}}").unwrap();
    let templates = crate::user_templates(dir.path()).unwrap();
    assert_eq!(templates[0].name, "Standup");
    // A template stored under .templates never appears in the note index.
    assert!(crate::index(dir.path()).unwrap().notes.is_empty());

    let note = crate::create_from_template(dir.path(), &templates[0].body, "Monday", 0).unwrap();
    assert!(note.content.contains("# Monday"));
    assert!(note.content.contains("Date: 1970-01-01"));

    // Built-in templates stamp a calendar date, not a raw integer.
    let built = crate::template(Template::Task, "Ship it", 0);
    assert!(built.contains("created: 1970-01-01"));
    assert_eq!(crate::iso_date(1_752_000_000), "2025-07-08");
}
