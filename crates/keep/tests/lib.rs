use super::*;

#[test]
fn round_trips_through_encryption() {
    let mut keep = Keep::create("hunter2").unwrap();
    keep.set(&Scope::Global, "openai", "sk-global");
    keep.set(&Scope::Project(7), "stripe", "sk_live_7");
    let bytes = keep.to_bytes().unwrap();

    // The ciphertext must not contain the plaintext secrets.
    assert!(!contains(&bytes, b"sk-global"));
    assert!(!contains(&bytes, b"sk_live_7"));

    let opened = Keep::unlock(&bytes, "hunter2").unwrap();
    assert_eq!(opened.get(&Scope::Global, "openai"), Some("sk-global"));
    assert_eq!(opened.get(&Scope::Project(7), "stripe"), Some("sk_live_7"));
}

#[test]
fn wrong_passphrase_is_rejected() {
    let mut keep = Keep::create("correct").unwrap();
    keep.set(&Scope::Global, "k", "v");
    let bytes = keep.to_bytes().unwrap();
    assert!(matches!(
        Keep::unlock(&bytes, "wrong"),
        Err(Error::WrongPassphrase)
    ));
    // Not-a-keep bytes are a format error, not a passphrase error.
    assert!(matches!(Keep::unlock(b"nope", "x"), Err(Error::BadFormat)));
}

#[test]
fn project_scope_overlays_global() {
    let mut keep = Keep::create("p").unwrap();
    keep.set(&Scope::Global, "key", "global-val");
    keep.set(&Scope::Project(7), "key", "project-7-val");
    keep.set(&Scope::Project(7), "only7", "sevens");

    // Project 7 sees its override and its own key, plus global fallbacks.
    assert_eq!(keep.resolve(Some(7), "key"), Some("project-7-val"));
    assert_eq!(keep.resolve(Some(7), "only7"), Some("sevens"));
    // A different project only gets the global value.
    assert_eq!(keep.resolve(Some(9), "key"), Some("global-val"));
    assert_eq!(keep.resolve(Some(9), "only7"), None);
    // A global-only caller never sees project keys.
    assert_eq!(keep.resolve(None, "key"), Some("global-val"));
    assert_eq!(keep.resolve(None, "only7"), None);
}

#[test]
fn set_remove_and_list() {
    let mut keep = Keep::create("p").unwrap();
    keep.set(&Scope::Global, "a", "1");
    keep.set(&Scope::Global, "b", "2");
    assert_eq!(keep.names(&Scope::Global), vec!["a", "b"]);
    assert!(keep.remove(&Scope::Global, "a"));
    assert!(!keep.remove(&Scope::Global, "a"));
    assert_eq!(keep.names(&Scope::Global), vec!["b"]);
    assert!(keep.names(&Scope::Project(1)).is_empty());
}

#[test]
fn updates_survive_a_resave() {
    let mut keep = Keep::create("pw").unwrap();
    keep.set(&Scope::Global, "a", "1");
    let keep = Keep::unlock(&keep.to_bytes().unwrap(), "pw").unwrap();
    let mut keep = keep;
    keep.set(&Scope::Global, "a", "2");
    keep.set(&Scope::Project(3), "c", "3");
    let reopened = Keep::unlock(&keep.to_bytes().unwrap(), "pw").unwrap();
    assert_eq!(reopened.get(&Scope::Global, "a"), Some("2"));
    assert_eq!(reopened.get(&Scope::Project(3), "c"), Some("3"));
}

/// A fresh nonce per encryption is the one property whose loss would be
/// catastrophic: GCM reuses a keystream if a nonce repeats under the same key,
/// so two saves of the same keep must never produce the same nonce.
#[test]
fn every_encryption_uses_a_fresh_nonce() {
    let mut keep = Keep::create("pw").unwrap();
    keep.set(&Scope::Global, "k", "v");

    let nonce_of =
        |b: &[u8]| b[MAGIC.len() + SALT_LEN..MAGIC.len() + SALT_LEN + NONCE_LEN].to_vec();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..32 {
        let bytes = keep.to_bytes().unwrap();
        // The salt is stable across saves; the nonce must not be.
        assert_eq!(&bytes[MAGIC.len()..MAGIC.len() + SALT_LEN], &keep.salt);
        assert!(seen.insert(nonce_of(&bytes)), "nonce reused across saves");
    }
}

/// Two keeps made from the same passphrase must not share a salt, or they would
/// share a derived key.
#[test]
fn each_keep_gets_its_own_salt() {
    let a = Keep::create("same").unwrap();
    let b = Keep::create("same").unwrap();
    assert_ne!(a.salt, b.salt);
}

/// GCM authenticates; a flipped ciphertext bit must fail rather than decrypt to
/// garbage.
#[test]
fn tampered_ciphertext_is_rejected() {
    let mut keep = Keep::create("pw").unwrap();
    keep.set(&Scope::Global, "k", "v");
    let good = keep.to_bytes().unwrap();

    // Flip a bit in the ciphertext body.
    let mut bad = good.clone();
    let last = bad.len() - 1;
    bad[last] ^= 0x01;
    assert!(matches!(
        Keep::unlock(&bad, "pw"),
        Err(Error::WrongPassphrase)
    ));

    // Swapping the nonce invalidates the tag too.
    let mut swapped = good.clone();
    swapped[MAGIC.len() + SALT_LEN] ^= 0xff;
    assert!(matches!(
        Keep::unlock(&swapped, "pw"),
        Err(Error::WrongPassphrase)
    ));

    // A truncated keep is a format error, not a silent empty keep.
    assert!(matches!(
        Keep::unlock(&good[..MAGIC.len() + 4], "pw"),
        Err(Error::BadFormat)
    ));
}

#[test]
fn saves_and_reopens_from_disk() {
    let dir = scratch("roundtrip");
    let path = dir.join("keep.enc");

    let mut keep = Keep::create("pw").unwrap();
    keep.set(&Scope::Global, "openai", "sk-global");
    keep.set(&Scope::Project(7), "stripe", "sk_live_7");
    keep.save(&path).unwrap();

    let opened = Keep::open(&path, "pw").unwrap();
    assert_eq!(opened.get(&Scope::Global, "openai"), Some("sk-global"));
    assert_eq!(opened.get(&Scope::Project(7), "stripe"), Some("sk_live_7"));
    assert!(matches!(
        Keep::open(&path, "wrong"),
        Err(Error::WrongPassphrase)
    ));

    let _ = std::fs::remove_dir_all(&dir);
}

/// The keep file must not be readable by other users, and `save` must not leave
/// its temp file behind.
#[test]
fn saved_keep_is_private_and_leaves_no_temp_files() {
    let dir = scratch("perms");
    let path = dir.join("keep.enc");
    let mut keep = Keep::create("pw").unwrap();
    keep.set(&Scope::Global, "k", "v");
    keep.save(&path).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "keep file must be owner-only");
    }

    // Re-save a few times; no `.tmp` spool may survive.
    for i in 0..3 {
        keep.set(&Scope::Global, "k", &format!("v{i}"));
        keep.save(&path).unwrap();
    }
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n != "keep.enc")
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp files left behind: {leftovers:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Overwriting an existing keep must replace it wholesale, not merge or corrupt.
#[test]
fn resave_replaces_the_previous_file() {
    let dir = scratch("resave");
    let path = dir.join("keep.enc");

    let mut keep = Keep::create("pw").unwrap();
    keep.set(&Scope::Global, "a", "1");
    keep.save(&path).unwrap();

    let mut keep = Keep::open(&path, "pw").unwrap();
    keep.remove(&Scope::Global, "a");
    keep.set(&Scope::Global, "b", "2");
    keep.save(&path).unwrap();

    let opened = Keep::open(&path, "pw").unwrap();
    assert_eq!(opened.get(&Scope::Global, "a"), None);
    assert_eq!(opened.get(&Scope::Global, "b"), Some("2"));

    let _ = std::fs::remove_dir_all(&dir);
}

/// `save` creates the parent directory (first run, before `~/.config/asylum`
/// exists).
#[test]
fn save_creates_missing_parent_directory() {
    let dir = scratch("mkdir");
    let path = dir.join("nested").join("deeper").join("keep.enc");
    let keep = Keep::create("pw").unwrap();
    keep.save(&path).unwrap();
    assert!(path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn all_values_spans_every_scope() {
    let mut keep = Keep::create("pw").unwrap();
    keep.set(&Scope::Global, "a", "global-secret");
    keep.set(&Scope::Project(1), "b", "project-1-secret");
    keep.set(&Scope::Project(2), "c", "project-2-secret");
    let mut vals = keep.all_values();
    vals.sort();
    assert_eq!(
        vals,
        vec!["global-secret", "project-1-secret", "project-2-secret"]
    );
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("asylum-keep-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
