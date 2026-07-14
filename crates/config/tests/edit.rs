use super::*;

const FILE: &str = r#"// Asylum settings.
{
    // The chrome theme.
    "theme": "dark",

    /* worktrees live here */
    "worktree_dir": ".asylum/worktrees",

    "editor": {
        "font_size": 13, // points
        "autosave": true
    },

    "default_agents": ["claude-code", "codex"]
}
"#;

#[test]
fn upsert_replaces_value_in_place() {
    let out = upsert(FILE, "theme", "\"light\"").unwrap();
    assert!(out.contains("\"theme\": \"light\""));
    // Every comment survives.
    assert!(out.contains("// Asylum settings."));
    assert!(out.contains("// The chrome theme."));
    assert!(out.contains("/* worktrees live here */"));
    assert!(out.contains("// points"));
    // Other keys untouched.
    assert!(out.contains("\"worktree_dir\": \".asylum/worktrees\""));
}

#[test]
fn upsert_replaces_nested_object_wholesale() {
    let out = upsert(FILE, "editor", "{ \"font_size\": 15 }").unwrap();
    assert!(out.contains("\"editor\": { \"font_size\": 15 }"));
    assert!(!out.contains("autosave"));
    assert!(out.contains("\"default_agents\": [\"claude-code\", \"codex\"]"));
}

#[test]
fn upsert_appends_missing_key() {
    let out = upsert(FILE, "max_parallel_runs", "8").unwrap();
    assert!(out.contains("\"max_parallel_runs\": 8"));
    // Appended after the last member with a separating comma.
    let agents = out.find("default_agents").unwrap();
    let added = out.find("max_parallel_runs").unwrap();
    assert!(added > agents);
    assert!(crate::load::load_str(&out).diagnostics.is_empty());
}

#[test]
fn upsert_into_empty_object() {
    let out = upsert("{\n}\n", "theme", "\"light\"").unwrap();
    assert!(out.contains("\"theme\": \"light\""));
    assert!(crate::load::load_str(&out).diagnostics.is_empty());
}

#[test]
fn upsert_refuses_non_object() {
    assert!(upsert("[1, 2]", "theme", "\"light\"").is_none());
    assert!(upsert("not json at all", "theme", "\"light\"").is_none());
}

#[test]
fn remove_middle_key_takes_line_and_comma() {
    let out = remove(FILE, "worktree_dir").unwrap();
    assert!(!out.contains("worktree_dir"));
    assert!(out.contains("\"theme\": \"dark\""));
    assert!(out.contains("\"editor\""));
    assert!(crate::load::load_str(&out).diagnostics.is_empty());
}

#[test]
fn remove_last_key_takes_preceding_comma() {
    let out = remove(FILE, "default_agents").unwrap();
    assert!(!out.contains("default_agents"));
    assert!(crate::load::load_str(&out).diagnostics.is_empty());
}

#[test]
fn remove_absent_key_is_noop() {
    assert_eq!(remove(FILE, "missing").unwrap(), FILE);
}

#[test]
fn edited_file_round_trips_through_load() {
    let out = upsert(FILE, "theme", "\"light\"").unwrap();
    let out = upsert(&out, "max_parallel_runs", "2").unwrap();
    let out = remove(&out, "default_agents").unwrap();
    let loaded = crate::load::load_str(&out);
    assert!(loaded.diagnostics.is_empty());
    assert_eq!(loaded.settings.theme, "light");
    assert_eq!(loaded.settings.max_parallel_runs, 2);
    assert!(loaded.settings.default_agents.is_empty());
}

fn temp_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("asylumedit{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    dir.join(name)
}

#[test]
fn set_key_seeds_missing_file_from_starter() {
    let path = temp_path("fresh.json");
    let _ = std::fs::remove_file(&path);
    set_key(&path, "theme", "\"light\"").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("// Asylum settings"));
    assert_eq!(crate::load::load_str(&text).settings.theme, "light");
}

#[test]
fn remove_key_restores_default() {
    let path = temp_path("reset.json");
    std::fs::write(&path, FILE).unwrap();
    remove_key(&path, "theme").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        crate::load::load_str(&text).settings.theme,
        crate::model::Settings::default().theme
    );
    assert!(text.contains("/* worktrees live here */"));
}

#[test]
fn set_key_refuses_broken_file() {
    let path = temp_path("broken.json");
    std::fs::write(&path, "[]").unwrap();
    assert!(set_key(&path, "theme", "\"light\"").is_err());
    // The broken file is left exactly as it was.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[]");
}

#[test]
fn ensure_file_writes_starter_once() {
    let path = temp_path("ensure.json");
    let _ = std::fs::remove_file(&path);
    ensure_file(&path).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), STARTER);
    std::fs::write(&path, "{ \"theme\": \"light\" }").unwrap();
    ensure_file(&path).unwrap();
    assert!(std::fs::read_to_string(&path).unwrap().contains("light"));
}
