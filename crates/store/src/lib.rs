//! SQLite persistence for the Agent Development Environment.
//!
//! The store holds the ADE's durable state: the **projects** you work in (git
//! repositories), the **tasks** you pose (a prompt, optionally fanned across
//! several agents), and the **runs** - one per agent working a task in its own
//! isolated worktree. It is deliberately synchronous (`rusqlite`, bundled
//! SQLite): the gpui app owns one [`Db`] and calls into it directly, running
//! the occasional heavy query on a background executor.
//!
//! Submodules:
//! - [`schema`] - connection open + idempotent migrations.
//! - [`model`] - the row types and their enums.
//! - [`project`], [`task`], [`run`] - CRUD for each entity.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};

pub mod account;
pub mod annotation;
pub mod control;
pub mod event;
pub mod followup;
pub mod lock;
pub mod model;
pub mod note;
pub mod notification;
pub mod project;
mod queue;
pub mod run;
pub mod runcheck;
mod schema;
pub mod search;
pub mod task;

pub use model::{
    Account, Annotation, ControlRequest, Event, Followup, NoteAttachment, NoteVault, NoteVaultMode,
    Notification, Project, QueueStatus, Run, RunCheck, RunStatus, SearchKind, SearchRecord, Side,
    Task, TaskStatus, Usage,
};

/// A store error: either the SQLite layer failed or a lookup found nothing.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("not found")]
    NotFound,
}

/// The result type used throughout the store.
pub type Result<T> = std::result::Result<T, Error>;

/// The database handle: a single owned SQLite connection plus the ADE's schema.
///
/// One writable [`Db`] per running app. Not `Sync` - wrap it in the app's state
/// cell and touch it from one place. For concurrent background reads (search),
/// open a second, read-only handle with [`Db::open_readonly`]: WAL mode lets it
/// run alongside the writer without blocking it.
pub struct Db {
    conn: Connection,
}

impl Db {
    /// Open (creating if needed) the database at `path` and run migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Open a private in-memory database - used by tests and ephemeral sessions.
    pub fn memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }

    /// Open an additional **read-only** handle to an on-disk database for
    /// concurrent background reads (project search runs on a worker thread).
    ///
    /// The app owns one writable [`Db`] on its UI thread; each background search
    /// opens one of these against the same file and reads without touching the
    /// writer. WAL mode (set on the primary handle by [`schema::migrate`]) lets
    /// these readers run in parallel with the writer and each other. This path
    /// deliberately does **not** migrate - a reader must never change schema - so
    /// the file must already exist and have been opened writably at least once.
    pub fn open_readonly(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Ok(Self { conn })
    }

    /// Borrow the underlying connection for the entity modules.
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
