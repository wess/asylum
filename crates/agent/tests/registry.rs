use super::*;
use config::CustomAgent;

#[test]
fn builtins_have_unique_ids() {
    let mut ids: Vec<&str> = builtins().iter().map(|a| a.id).collect();
    ids.sort_unstable();
    let count = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), count, "duplicate agent ids");
}

#[test]
fn catalog_is_broad() {
    // Ensure the catalog is broad (30+) and includes the
    // marquee ones.
    assert!(
        builtins().len() >= 30,
        "catalog too small: {}",
        builtins().len()
    );
    for id in [
        "claude-code",
        "codex",
        "grok",
        "cursor-agent",
        "copilot",
        "gemini",
        "cline",
        "devin",
    ] {
        assert!(find(id).is_some(), "missing built-in agent {id}");
    }
}

#[test]
fn find_known_and_unknown() {
    assert_eq!(find("claude-code").unwrap().program, "claude");
    assert!(find("nope").is_none());
}

#[test]
fn custom_agent_appends_to_catalog() {
    let custom = vec![CustomAgent {
        id: "my-agent".into(),
        name: "My Agent".into(),
        icon: "★".into(),
        program: "myagent".into(),
        args: vec!["{prompt}".into()],
        delivery: "arg".into(),
    }];
    let cat = catalog(&custom);
    assert_eq!(cat.len(), builtins().len() + 1);
    let mine = resolve("my-agent", &custom).unwrap();
    assert!(!mine.builtin);
    assert_eq!(mine.name, "My Agent");
}

#[test]
fn custom_agent_overrides_builtin_in_place() {
    let custom = vec![CustomAgent {
        id: "codex".into(),
        name: "Codex (patched)".into(),
        icon: "◆".into(),
        program: "codex-nightly".into(),
        args: vec!["run".into(), "{prompt}".into()],
        delivery: "arg".into(),
    }];
    let cat = catalog(&custom);
    assert_eq!(
        cat.len(),
        builtins().len(),
        "override should not grow the catalog"
    );
    let codex = resolve("codex", &custom).unwrap();
    assert_eq!(codex.program, "codex-nightly");
    assert!(!codex.builtin);
}

#[test]
fn delivery_parse() {
    assert_eq!(Delivery::parse("stdin"), Delivery::Stdin);
    assert_eq!(Delivery::parse("arg"), Delivery::Arg);
    assert_eq!(Delivery::parse(""), Delivery::Arg);
}

#[test]
fn builtin_icons_are_not_pictographic_emoji() {
    // Icons are monochrome symbol glyphs, not colour emoji; guard against a
    // pictographic emoji (U+1F000 and up) creeping back in.
    for a in builtins() {
        assert!(
            a.icon.chars().all(|c| (c as u32) < 0x1F000),
            "agent {} has an emoji icon: {:?}",
            a.id,
            a.icon
        );
    }
}
