//! An encrypted, scoped secret keep.
//!
//! Secret values are stored in a single file encrypted with AES-256-GCM under a
//! key derived from a passphrase (PBKDF2-HMAC-SHA256). Unlocking decrypts the
//! file into memory; from then on values live only in the process's heap (and
//! are re-encrypted on save). The keep is **scoped**: a secret belongs to the
//! [`Scope::Global`] keep or a [`Scope::Project`] keep, and resolution overlays a
//! project's keep on top of the global one — so a project can override or add
//! keys without touching global ones.
//!
//! Pure and gpui-free: file I/O is a thin convenience over the byte codec, so the
//! crypto and scoping are unit-tested without disk.

use std::collections::BTreeMap;
use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// File magic + version.
const MAGIC: &[u8] = b"ASYLKEEP1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
/// PBKDF2 iteration count (OWASP guidance for PBKDF2-HMAC-SHA256).
const ROUNDS: u32 = 600_000;

/// Which keep a secret belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Available to every project.
    Global,
    /// Available only to the given project (overlays the global keep).
    Project(i64),
}

impl Scope {
    /// The stable string key used inside the encrypted map.
    pub fn key(&self) -> String {
        match self {
            Scope::Global => "global".to_string(),
            Scope::Project(id) => format!("project:{id}"),
        }
    }
}

/// Errors from opening or decrypting a keep.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a keep file")]
    BadFormat,
    #[error("wrong passphrase or corrupt keep")]
    WrongPassphrase,
    #[error("keep contents are malformed: {0}")]
    Malformed(String),
    #[error("i/o error: {0}")]
    Io(String),
    #[error("random source unavailable: {0}")]
    Random(String),
}

/// An unlocked keep held in memory. Encrypts back to bytes on demand.
pub struct Keep {
    /// scope key -> (secret name -> value).
    data: BTreeMap<String, BTreeMap<String, String>>,
    /// The derived key, kept so `to_bytes` can re-encrypt without the passphrase.
    key: Zeroizing<[u8; 32]>,
    salt: [u8; SALT_LEN],
}

#[derive(Deserialize, Default)]
struct Plain {
    scopes: BTreeMap<String, BTreeMap<String, String>>,
}

/// The serialize side of [`Plain`], borrowing rather than cloning: copying every
/// secret into a throwaway map on each save would leave one more plaintext
/// duplicate in the heap for no reason.
#[derive(Serialize)]
struct PlainRef<'a> {
    scopes: &'a BTreeMap<String, BTreeMap<String, String>>,
}

impl Keep {
    /// Create a new, empty keep locked with `passphrase` (a fresh random salt).
    pub fn create(passphrase: &str) -> Result<Self, Error> {
        let mut salt = [0u8; SALT_LEN];
        fill(&mut salt)?;
        Ok(Self {
            data: BTreeMap::new(),
            key: derive(passphrase, &salt),
            salt,
        })
    }

    /// Unlock an existing keep from its file `bytes` using `passphrase`.
    pub fn unlock(bytes: &[u8], passphrase: &str) -> Result<Self, Error> {
        if bytes.len() < MAGIC.len() + SALT_LEN + NONCE_LEN || &bytes[..MAGIC.len()] != MAGIC {
            return Err(Error::BadFormat);
        }
        let mut off = MAGIC.len();
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&bytes[off..off + SALT_LEN]);
        off += SALT_LEN;
        let nonce = &bytes[off..off + NONCE_LEN];
        off += NONCE_LEN;
        let ciphertext = &bytes[off..];

        let key = derive(passphrase, &salt);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&*key));
        // Every secret in the keep passes through this buffer in the clear;
        // wipe it rather than leave it in freed heap (and so out of core dumps).
        let plaintext = Zeroizing::new(
            cipher
                .decrypt(Nonce::from_slice(nonce), ciphertext)
                .map_err(|_| Error::WrongPassphrase)?,
        );
        let plain: Plain =
            serde_json::from_slice(&plaintext).map_err(|e| Error::Malformed(e.to_string()))?;
        Ok(Self {
            data: plain.scopes,
            key,
            salt,
        })
    }

    /// Encrypt the current contents to the keep file format (fresh nonce).
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let plain = PlainRef { scopes: &self.data };
        let plaintext = Zeroizing::new(
            serde_json::to_vec(&plain).map_err(|e| Error::Malformed(e.to_string()))?,
        );
        let mut nonce = [0u8; NONCE_LEN];
        fill(&mut nonce)?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&*self.key));
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
            .map_err(|_| Error::Malformed("encryption failed".into()))?;
        let mut out = Vec::with_capacity(MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.salt);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Convenience: unlock from a file on disk.
    pub fn open(path: &Path, passphrase: &str) -> Result<Self, Error> {
        let bytes = std::fs::read(path).map_err(|e| Error::Io(e.to_string()))?;
        Self::unlock(&bytes, passphrase)
    }

    /// Convenience: encrypt and write to a file (0600 where supported).
    ///
    /// The write is atomic: the ciphertext goes to a fresh sibling temp file,
    /// is flushed to disk, and is then renamed over `path`. A truncating
    /// in-place write would be unrecoverable here - the keep has no backup and
    /// no journal, so a crash, a full disk, or a power loss partway through
    /// would destroy every secret it holds. Renaming means an interrupted save
    /// leaves the previous keep intact.
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        let bytes = self.to_bytes()?;
        let dir = path.parent().filter(|p| !p.as_os_str().is_empty());
        let dir = dir.unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(dir).map_err(|e| Error::Io(e.to_string()))?;

        // A random suffix keeps concurrent saves (and a stale temp from an
        // earlier crash) from colliding; `create_new` below refuses to reuse one.
        let mut suffix = [0u8; 8];
        fill(&mut suffix)?;
        let tmp = dir.join(format!("keep.{}.tmp", hex(&suffix)));

        let write = (|| -> std::io::Result<()> {
            use std::io::Write;
            // Created 0600 rather than chmod'd afterwards: a widened window,
            // however brief, is a window.
            let mut f = create_private(&tmp)?;
            f.write_all(&bytes)?;
            // Durable before the rename, so the rename can never publish a file
            // whose contents have not landed.
            f.sync_all()
        })();
        if let Err(e) = write {
            let _ = std::fs::remove_file(&tmp);
            return Err(Error::Io(e.to_string()));
        }

        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(Error::Io(e.to_string()));
        }
        restrict_permissions(path);
        sync_dir(dir);
        Ok(())
    }

    /// Set a secret in `scope`.
    pub fn set(&mut self, scope: &Scope, name: &str, value: &str) {
        self.data
            .entry(scope.key())
            .or_default()
            .insert(name.to_string(), value.to_string());
    }

    /// Remove a secret from `scope`. Returns whether it existed.
    pub fn remove(&mut self, scope: &Scope, name: &str) -> bool {
        self.data
            .get_mut(&scope.key())
            .map(|m| m.remove(name).is_some())
            .unwrap_or(false)
    }

    /// A secret's value in a specific scope (no overlay).
    pub fn get(&self, scope: &Scope, name: &str) -> Option<&str> {
        self.data.get(&scope.key())?.get(name).map(String::as_str)
    }

    /// The secret names in `scope` (sorted).
    pub fn names(&self, scope: &Scope) -> Vec<String> {
        self.data
            .get(&scope.key())
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Whether `scope` has a secret named `name`.
    pub fn has(&self, scope: &Scope, name: &str) -> bool {
        self.data
            .get(&scope.key())
            .is_some_and(|m| m.contains_key(name))
    }

    /// Every secret value across all scopes - for transcript redaction only.
    pub fn all_values(&self) -> Vec<String> {
        self.data
            .values()
            .flat_map(|m| m.values().cloned())
            .collect()
    }

    /// Every scope holding at least one secret, each paired with its sorted
    /// secret names. The scope key is the stable string form (`global`,
    /// `project:<id>`); values are never returned. Empty scopes are omitted, so
    /// a UI can list what the keep actually holds without knowing which projects
    /// exist. Names only - listing a keep must never surface a value.
    pub fn scopes(&self) -> Vec<(String, Vec<String>)> {
        self.data
            .iter()
            .filter(|(_, secrets)| !secrets.is_empty())
            .map(|(scope, secrets)| (scope.clone(), secrets.keys().cloned().collect()))
            .collect()
    }

    /// Resolve `name` for an agent in `project` (or `None` for a global-only
    /// caller): the project's keep wins, falling back to the global keep.
    pub fn resolve(&self, project: Option<i64>, name: &str) -> Option<&str> {
        if let Some(id) = project {
            if let Some(v) = self.get(&Scope::Project(id), name) {
                return Some(v);
            }
        }
        self.get(&Scope::Global, name)
    }
}

impl Drop for Keep {
    /// Wipe the secret values on the way out. The derived key already zeroizes
    /// itself; the values it guards deserve the same, so an unlocked keep does
    /// not outlive the process in freed heap or a core dump.
    fn drop(&mut self) {
        use zeroize::Zeroize;
        for scope in self.data.values_mut() {
            for value in scope.values_mut() {
                value.zeroize();
            }
        }
    }
}

/// Derive a 32-byte key from a passphrase and salt (PBKDF2-HMAC-SHA256).
fn derive(passphrase: &str, salt: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0u8; 32]);
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(passphrase.as_bytes(), salt, ROUNDS, &mut *key);
    key
}

fn fill(buf: &mut [u8]) -> Result<(), Error> {
    getrandom::fill(buf).map_err(|e| Error::Random(e.to_string()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Create a file that no other user can read, failing if it already exists.
#[cfg(unix)]
fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

/// Flush the directory entry, so a completed `save` survives a power loss.
/// Best-effort: not every filesystem allows opening a directory for sync.
#[cfg(unix)]
fn sync_dir(dir: &Path) {
    if let Ok(d) = std::fs::File::open(dir) {
        let _ = d.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) {}

#[cfg(unix)]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) {}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
