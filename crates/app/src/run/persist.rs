//! Incremental transcript persistence: decide when a live run's transcript is
//! worth writing to the store, and bound what is retained.
//!
//! The open terminal pane renders the live in-memory [`TermView`], so the stored
//! transcript is only ever read back for restart recovery and finished-run
//! history (see `fleet` and `state::review`). That lets persistence be lazy:
//! rather than rewrite a run's entire — and steadily growing — transcript to
//! SQLite on every ~1s wakeup (quadratic write amplification over a run's life,
//! multiplied by every live agent), a snapshot is written only when the
//! transcript has actually changed, at most once every [`CADENCE_SECS`] seconds,
//! and always capped to [`BYTE_CAP`] bytes so a runaway agent cannot bloat the
//! row. Activity classification keeps riding the live in-memory text every tick
//! — it needs freshness; persistence does not.
//!
//! [`TermView`]: libsinclair::termview::TermView

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};

/// Byte budget for a persisted transcript. Only the last `BYTE_CAP` bytes are
/// retained (rounded to a char boundary), behind a one-line note — enough tail
/// to review or recover a run without letting the row grow without bound.
pub const BYTE_CAP: usize = 256 * 1024;

/// Minimum seconds between two lazy snapshots of the same run. The live pane
/// reads the terminal directly, so a stored copy that lags by a few seconds is
/// purely a recovery/history concern.
pub const CADENCE_SECS: i64 = 5;

/// Bytes at the tail folded into the change-detection fingerprint. Terminal
/// output is append-mostly, so a change to the last few KB (together with the
/// total length) tells "grew or changed" from "identical" far more cheaply than
/// a full compare — and still catches a redraw that shifts a full scrollback,
/// where the length holds steady but the tail turns over.
const TAIL_HASH_BYTES: usize = 8 * 1024;

/// A record of the last transcript persisted for one run: when it was written,
/// and a cheap fingerprint (total length + tail hash) so the next tick can tell
/// whether the transcript has since changed. Kept module-side, keyed by run id,
/// so the `Root` entity (state.rs) is left untouched — mirroring how the
/// Accounts surface caches its probes.
#[derive(Clone, Copy)]
pub struct Mark {
    saved_at: i64,
    len: usize,
    hash: u64,
}

fn marks() -> &'static Mutex<HashMap<i64, Mark>> {
    static MARKS: OnceLock<Mutex<HashMap<i64, Mark>>> = OnceLock::new();
    MARKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Round `at` up to the next UTF-8 char boundary in `text` (at most three
/// steps), so slicing a multi-byte transcript never splits a code point.
fn char_boundary(text: &str, mut at: usize) -> usize {
    while at < text.len() && !text.is_char_boundary(at) {
        at += 1;
    }
    at
}

/// A cheap fingerprint of a transcript: its byte length and a hash of its last
/// [`TAIL_HASH_BYTES`] bytes.
fn fingerprint(text: &str) -> (usize, u64) {
    let start = char_boundary(text, text.len().saturating_sub(TAIL_HASH_BYTES));
    let mut hasher = DefaultHasher::new();
    text[start..].hash(&mut hasher);
    (text.len(), hasher.finish())
}

/// Whether `text` differs from the fingerprint captured in `mark`.
fn changed(mark: &Mark, text: &str) -> bool {
    let (len, hash) = fingerprint(text);
    mark.len != len || mark.hash != hash
}

/// The pure persistence decision for one tick: write on the first snapshot,
/// then only once the cadence has elapsed *and* the transcript has actually
/// changed since the last write recorded in `mark`.
pub fn should_persist(mark: Option<Mark>, text: &str, now: i64) -> bool {
    match mark {
        None => true,
        Some(mark) => now - mark.saved_at >= CADENCE_SECS && changed(&mark, text),
    }
}

/// Cap a transcript to the last [`BYTE_CAP`] bytes for storage. Shorter text is
/// returned unchanged; longer text keeps its tail (rounded to a char boundary)
/// behind a one-line note, so a recovered run reads as trimmed rather than
/// silently truncated mid-stream.
pub fn cap(text: &str) -> String {
    if text.len() <= BYTE_CAP {
        return text.to_string();
    }
    let start = char_boundary(text, text.len() - BYTE_CAP);
    format!(
        "[earlier terminal output trimmed; showing the last {} KB]\n{}",
        BYTE_CAP / 1024,
        &text[start..]
    )
}

/// Whether run `run_id`'s transcript should be persisted this tick, given the
/// current transcript and second.
pub fn due(run_id: i64, text: &str, now: i64) -> bool {
    let mark = marks()
        .lock()
        .ok()
        .and_then(|marks| marks.get(&run_id).copied());
    should_persist(mark, text, now)
}

/// Record that run `run_id`'s transcript was just persisted — its fingerprint
/// and the current second — so the next tick can tell whether it has changed.
pub fn record(run_id: i64, text: &str, now: i64) {
    let (len, hash) = fingerprint(text);
    if let Ok(mut marks) = marks().lock() {
        marks.insert(
            run_id,
            Mark {
                saved_at: now,
                len,
                hash,
            },
        );
    }
}

/// Drop a run's persistence bookkeeping at a terminal transition, so the
/// module-side map does not accumulate an entry per run launched this session.
pub fn forget(run_id: i64) {
    if let Ok(mut marks) = marks().lock() {
        marks.remove(&run_id);
    }
}

#[cfg(test)]
#[path = "../../tests/run/persist.rs"]
mod tests;
