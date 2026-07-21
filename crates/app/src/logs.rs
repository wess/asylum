//! Structured logging: a daily-rolling file under `<data dir>/logs`, mirrored
//! to stderr in debug builds only. The level comes from `ASYLUM_LOG` (e.g.
//! `ASYLUM_LOG=debug`), defaulting to `info`.
//!
//! `init` is called once, first thing in `main`, and returns the worker guard
//! that must stay alive for the process lifetime - dropping it early can
//! silently discard whatever log lines are still buffered.

use std::io::Write;
use std::path::{Path, PathBuf};

use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::Layer as _;

/// Overrides the default `info` level, e.g. `ASYLUM_LOG=debug`.
const LEVEL_VAR: &str = "ASYLUM_LOG";

/// Daily log files beyond this count are pruned by `tracing-appender`.
const MAX_LOG_FILES: usize = 7;

/// Install the global tracing subscriber and return the guard that keeps the
/// non-blocking file writer alive.
pub fn init() -> WorkerGuard {
    let dir = default_dir();
    let level = parse_level(std::env::var(LEVEL_VAR).ok().as_deref());

    // The appender's max_log_files pruner reads the directory at build time,
    // which stderr-warns on a first run where it doesn't exist yet.
    let _ = std::fs::create_dir_all(&dir);
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("asylum.log")
        .max_log_files(MAX_LOG_FILES)
        .build(&dir)
        .map(|appender| Box::new(appender) as Box<dyn Write + Send>)
        .unwrap_or_else(|error| {
            eprintln!("logs: could not open {}: {error}", dir.display());
            Box::new(std::io::sink())
        });
    let (writer, guard) = tracing_appender::non_blocking(appender);

    // ANSI escapes never belong in a file meant to be grepped or attached to
    // a bug report, regardless of whether the "ansi" feature ends up compiled
    // in via some other dependency.
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_filter(level);
    let registry = tracing_subscriber::registry().with(file_layer);

    #[cfg(debug_assertions)]
    registry
        .with(tracing_subscriber::fmt::layer().with_filter(level))
        .init();
    #[cfg(not(debug_assertions))]
    registry.init();

    guard
}

/// Where log files live: `<data dir>/logs`, alongside the on-disk store
/// (`state::Root::db_path`) and the plugins directory (`plugin::default_dir`).
pub fn default_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from(".local/share"));
    join(&base.join("asylum"))
}

/// `<data dir>/logs`, split out of `default_dir` so the join itself is
/// testable without touching the environment.
fn join(data_dir: &Path) -> PathBuf {
    data_dir.join("logs")
}

/// Parse `ASYLUM_LOG` into a level, defaulting to `info` when unset, blank,
/// or unrecognized. Tracing's own parser maps a blank string to `error`
/// (useful for its own CLI conventions, not this app's), so blank is filtered
/// out before delegating.
fn parse_level(raw: Option<&str>) -> LevelFilter {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok())
        .unwrap_or(LevelFilter::INFO)
}

#[cfg(test)]
#[path = "../tests/logs.rs"]
mod tests;
