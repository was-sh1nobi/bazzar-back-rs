//! In-memory global mutation version counter (spec §6, Node `db.ts:125-129`).
//!
//! Bumped on every AH mutation (add/list/buy/retrieve). `/items/poll` long-polls
//! on this counter: it busy-waits until `version > since`, then re-runs the
//! main items query. In-memory only — resets on restart, single-instance,
//! exactly like the Node reference implementation.
//!
//! When new mutation endpoints (/list, /items/add, /items/buy, ...) are ported,
//! they must call [`bump_version`] after a successful write.

use std::sync::atomic::{AtomicI64, Ordering};

static MUTATION_VERSION: AtomicI64 = AtomicI64::new(0);

/// Increment and return the new version (first bump returns 1).
pub fn bump_version() -> i64 {
    MUTATION_VERSION.fetch_add(1, Ordering::SeqCst) + 1
}

/// Current mutation version, read into every items envelope.
pub fn get_version() -> i64 {
    MUTATION_VERSION.load(Ordering::SeqCst)
}
