//! The panic hook: chains the previous hook (default stderr output stays
//! unchanged), then writes a crash report - message, location, backtrace, app
//! version, and OS - to `<data dir>/logs/crash-<unix seconds>.log` and
//! through tracing.

use std::any::Any;
use std::backtrace::Backtrace;

use crate::logs;

/// Install the panic hook. Chains the previous hook rather than replacing it,
/// so interactive stderr output is unchanged; called once, early in `main`.
pub fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);

        let location = info
            .location()
            .map(ToString::to_string)
            .unwrap_or_else(|| "unknown location".to_string());
        let report = format_report(&Panic {
            message: &payload_message(info.payload()),
            location: &location,
            backtrace: &Backtrace::force_capture().to_string(),
            version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
        });

        tracing::error!("{report}");
        if let Err(error) = save(&report) {
            eprintln!("crash: could not save the crash report: {error}");
        }
    }));
}

/// The plain facts of one panic, already pulled out of `PanicHookInfo` and
/// `Backtrace` so a report can be built - and tested - without a real panic.
struct Panic<'a> {
    message: &'a str,
    location: &'a str,
    backtrace: &'a str,
    version: &'a str,
    os: &'a str,
}

/// Render a crash report as plain text.
fn format_report(panic: &Panic) -> String {
    format!(
        "Asylum {version} crashed on {os}\nlocation: {location}\nmessage: {message}\n\nbacktrace:\n{backtrace}\n",
        version = panic.version,
        os = panic.os,
        location = panic.location,
        message = panic.message,
        backtrace = panic.backtrace,
    )
}

/// A panic payload is almost always a `&str` or `String` (any `panic!()` in
/// 2021+ produces one); anything else has no useful text to show.
fn payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

/// Write `report` to `<data dir>/logs/crash-<unix seconds>.log`.
fn save(report: &str) -> std::io::Result<()> {
    let dir = logs::default_dir();
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(format!("crash-{}.log", unix_now())), report)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "../tests/crash.rs"]
mod tests;
