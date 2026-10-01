//! SQLite persistence. `open` connects (WAL, foreign keys) and applies the
//! hand-written migrations in `migrations/`. Every entity module is a set of
//! free functions over a `&Pool`.

pub mod approvals;
pub mod bots;
pub mod chats;
pub mod events;
pub mod memories;
pub mod messages;
pub mod notifications;
pub mod plugins;
pub mod routines;
pub mod rules;
pub mod runs;
pub mod secrets;
pub mod sections;
pub mod skills;
pub mod slack;
pub mod state;
pub mod teams;
pub mod templates;
pub mod usage;

use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::path::Path;
use std::time::Duration;

pub use sqlx::SqlitePool as Pool;

pub use approvals::Approval;
pub use bots::Bot;
pub use chats::Chat;
pub use memories::Memory;
pub use messages::Message;
pub use notifications::Notification;
pub use plugins::{Account, Plugin};
pub use routines::Routine;
pub use rules::Rule;
pub use runs::Run;
pub use secrets::Secret;
pub use sections::Section;
pub use skills::Skill;
pub use teams::{Team, TeamMember};
pub use templates::Template;

pub async fn open(path: &Path) -> Result<Pool> {
  if let Some(dir) = path.parent() {
    std::fs::create_dir_all(dir)?;
  }
  let opts = SqliteConnectOptions::new()
    .filename(path)
    .create_if_missing(true)
    .foreign_keys(true)
    .busy_timeout(Duration::from_secs(5))
    .journal_mode(SqliteJournalMode::Wal);
  let pool = SqlitePoolOptions::new()
    .max_connections(8)
    .connect_with(opts)
    .await?;
  sqlx::migrate!("./migrations").run(&pool).await?;
  Ok(pool)
}

/// An in-memory database with the schema applied, for tests.
pub async fn memory() -> Result<Pool> {
  let opts = SqliteConnectOptions::new()
    .filename(":memory:")
    .foreign_keys(true);
  let pool = SqlitePoolOptions::new()
    .max_connections(1)
    .connect_with(opts)
    .await?;
  sqlx::migrate!("./migrations").run(&pool).await?;
  Ok(pool)
}

pub fn now() -> i64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_millis() as i64)
    .unwrap_or(0)
}

pub fn newid() -> String {
  uuid::Uuid::new_v4().simple().to_string()
}
