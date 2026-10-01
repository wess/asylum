use super::*;

#[test]
fn accepts_and_copies_with_unique_names() {
  let src = tempfile::tempdir().unwrap();
  let ws = tempfile::tempdir().unwrap();
  let f = src.path().join("notes.md");
  std::fs::write(&f, "# hi").unwrap();
  let a = ingest(ws.path(), std::slice::from_ref(&f)).unwrap();
  let b = ingest(ws.path(), &[f]).unwrap();
  match (&a[0], &b[0]) {
    (Part::Attachment { path: pa, mime, .. }, Part::Attachment { path: pb, .. }) => {
      assert_ne!(pa, pb);
      assert_eq!(mime, "text/markdown");
      assert!(pb.ends_with("notes 2.md"));
    }
    _ => panic!(),
  }
}

#[test]
fn rejects_bad_files() {
  let src = tempfile::tempdir().unwrap();
  let empty = src.path().join("e.txt");
  std::fs::write(&empty, "").unwrap();
  assert!(check(&empty).unwrap_err().to_string().contains("empty"));
  let bin = src.path().join("x.bin");
  std::fs::write(&bin, [0u8, 1, 2, 0]).unwrap();
  assert!(check(&bin).unwrap_err().to_string().contains("type"));
  let pdf = src.path().join("locked.pdf");
  std::fs::write(&pdf, b"%PDF-1.7 /Encrypt 5 0 R").unwrap();
  assert!(check(&pdf).unwrap_err().to_string().contains("password"));
  let many: Vec<PathBuf> = (0..7).map(|i| src.path().join(format!("{i}.txt"))).collect();
  assert!(ingest(src.path(), &many).is_err());
}
