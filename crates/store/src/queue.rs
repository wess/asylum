//! Shared retry policy for the app's drain queues (follow-ups, control
//! requests). Both tables carry the same lifecycle columns - `status`,
//! `attempts`, `last_error`, `claimed_at`, `completed_at`, `next_attempt_at` -
//! so the claim/complete/fail/recover SQL is identical up to the table name.
//! This module holds the policy constants and the backoff curve; the per-queue
//! modules ([`crate::followup`], [`crate::control`]) hold the typed reads.

/// Attempts allowed before a transient failure becomes a terminal `failed` row.
pub(crate) const MAX_ATTEMPTS: i64 = 5;

/// Base retry delay in seconds. The wait before attempt *n* is
/// `BASE * 2^(n-1)`, capped at [`BACKOFF_CAP_SECS`].
const BACKOFF_BASE_SECS: i64 = 5;

/// Ceiling on the retry delay, so backoff never parks work for too long.
const BACKOFF_CAP_SECS: i64 = 300;

/// A `running` row whose claim is older than this was orphaned by a crash and is
/// recovered on the next drain.
pub(crate) const STALE_RUNNING_SECS: i64 = 120;

/// Seconds to wait before the next attempt, given how many attempts have already
/// been made. `attempts` is the post-increment count on the row (>= 1 after a
/// claim), so the first retry waits `BASE`.
pub(crate) fn backoff_secs(attempts: i64) -> i64 {
    let shift = (attempts.max(1) - 1).min(16) as u32;
    BACKOFF_BASE_SECS
        .saturating_mul(1i64 << shift)
        .min(BACKOFF_CAP_SECS)
}

#[cfg(test)]
#[path = "../tests/queue.rs"]
mod tests;
