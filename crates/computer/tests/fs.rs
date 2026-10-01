use super::*;

#[test]
fn resolve_confines_to_home() {
  let home = Path::new("/data/bots/a/home");
  assert_eq!(resolve(home, "notes/a.md").unwrap(), home.join("notes/a.md"));
  assert_eq!(resolve(home, "~/x").unwrap(), home.join("x"));
  assert_eq!(resolve(home, "/data/bots/a/home/y").unwrap(), home.join("y"));
  assert!(resolve(home, "../../b/home").is_err());
  assert!(resolve(home, "/etc/passwd").is_err());
}

#[test]
fn write_edit_search_remove() {
  let dir = tempfile::tempdir().unwrap();
  let home = dir.path();
  write(home, "docs/plan.md", "alpha\nbeta\n").unwrap();
  edit(home, "docs/plan.md", "beta", "gamma").unwrap();
  assert_eq!(read(home, "docs/plan.md").unwrap(), "alpha\ngamma\n");
  assert!(edit(home, "docs/plan.md", "zzz", "y").is_err());
  let hits = search(home, "~", "gamma", 10).unwrap();
  assert_eq!(hits, vec!["~/docs/plan.md:2: gamma"]);
  let listing = list(home, "~").unwrap();
  assert!(listing[0].dir);
  assert!(remove(home, "~").is_err());
  remove(home, "docs").unwrap();
  assert!(list(home, "~").unwrap().is_empty());
}
