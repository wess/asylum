use super::*;
use std::process::id;
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

fn scratch() -> std::path::PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("asylum-plugins-{}-{n}", id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write_plugin(root: &std::path::Path, name: &str, manifest: &str) {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(MANIFEST), manifest).unwrap();
}

#[test]
fn missing_dir_is_empty() {
    let installed = load_dir(std::path::Path::new("/no/such/asylum/plugins"));
    assert!(installed.plugins.is_empty());
    assert!(installed.diagnostics.is_empty());
}

#[test]
fn loads_good_skips_bad() {
    let root = scratch();
    write_plugin(&root, "good", "id=\"good\"\nname=\"Good\"\n");
    write_plugin(&root, "bad", "name=\"NoId\"\n");
    // A directory without a manifest is silently ignored.
    std::fs::create_dir_all(root.join("empty")).unwrap();

    let installed = load_dir(&root);
    assert_eq!(installed.plugins.len(), 1);
    assert_eq!(installed.plugins[0].id, "good");
    assert_eq!(installed.diagnostics.len(), 1);
    assert!(installed.diagnostics[0].path.ends_with("plugin.toml"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn command_lookup_by_action_id() {
    let root = scratch();
    write_plugin(
        &root,
        "p",
        "id=\"p\"\nname=\"P\"\n[[command]]\nid=\"go\"\ntitle=\"Go\"\nrun=\"go\"\n",
    );
    let installed = load_dir(&root);
    let id = crate::action_id("p", "go");
    let (plugin, cmd) = crate::command(&installed.plugins, &id).unwrap();
    assert_eq!(plugin.id, "p");
    assert_eq!(cmd.title, "Go");
    assert!(crate::command(&installed.plugins, "p/missing").is_none());

    let _ = std::fs::remove_dir_all(&root);
}
