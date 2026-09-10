//! Deterministic work counters, behind the `profile` feature.
//!
//! A clock is the wrong primary instrument for most bricks in this crate. It
//! needs a quiet machine, and a laptop under load reads the *same binary* half
//! as fast as it did an hour earlier -- enough to swamp a change worth a third
//! of a parse. A count of the work a brick removed has none of those problems:
//! it is exact, identical on every machine under any load, needs one run rather
//! than twenty interleaved pairs, and it says whether the optimisation is even
//! *plugged in* -- which a byte-identical output gate cannot, because a fast
//! path that silently stopped being taken still produces the right answer.
//!
//! So the order is: count the work, then confirm with the clock when the box is
//! quiet. Never the reverse.
//!
//! Compiled out entirely without `profile`. With it, these add an atomic
//! increment to some very hot paths, so **a build with them on must never
//! produce a quoted timing**.

#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, Ordering};

/// Calls to `Read::peek`.
pub static PEEK: AtomicU64 = AtomicU64::new(0);
/// Calls to `Read::next`.
pub static NEXT: AtomicU64 = AtomicU64::new(0);
/// Calls to `Read::discard`.
pub static DISCARD: AtomicU64 = AtomicU64::new(0);
/// Calls to `Read::skip_whitespace` -- whitespace runs considered.
pub static WS_RUNS: AtomicU64 = AtomicU64::new(0);
/// Insignificant whitespace bytes skipped.
pub static WS_BYTES: AtomicU64 = AtomicU64::new(0);
/// Calls to `Read::take_8_digits`.
pub static D8_CALLS: AtomicU64 = AtomicU64::new(0);
/// Calls to `Read::take_8_digits` that consumed eight digits.
pub static D8_HITS: AtomicU64 = AtomicU64::new(0);

/// A reading of every counter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Counters {
    /// Calls to `Read::peek`.
    pub peek: u64,
    /// Calls to `Read::next`.
    pub next: u64,
    /// Calls to `Read::discard`.
    pub discard: u64,
    /// Whitespace runs considered.
    pub ws_runs: u64,
    /// Insignificant whitespace bytes skipped.
    pub ws_bytes: u64,
    /// Calls to `take_8_digits`.
    pub d8_calls: u64,
    /// Calls to `take_8_digits` that consumed eight digits.
    pub d8_hits: u64,
}

/// Read every counter.
#[must_use]
pub fn snapshot() -> Counters {
    Counters {
        peek: PEEK.load(Ordering::Relaxed),
        next: NEXT.load(Ordering::Relaxed),
        discard: DISCARD.load(Ordering::Relaxed),
        ws_runs: WS_RUNS.load(Ordering::Relaxed),
        ws_bytes: WS_BYTES.load(Ordering::Relaxed),
        d8_calls: D8_CALLS.load(Ordering::Relaxed),
        d8_hits: D8_HITS.load(Ordering::Relaxed),
    }
}

/// Zero every counter.
pub fn reset() {
    for c in [
        &PEEK, &NEXT, &DISCARD, &WS_RUNS, &WS_BYTES, &D8_CALLS, &D8_HITS,
    ] {
        c.store(0, Ordering::Relaxed);
    }
}

impl Counters {
    /// What happened between two readings.
    #[must_use]
    pub fn since(self, earlier: Counters) -> Counters {
        Counters {
            peek: self.peek - earlier.peek,
            next: self.next - earlier.next,
            discard: self.discard - earlier.discard,
            ws_runs: self.ws_runs - earlier.ws_runs,
            ws_bytes: self.ws_bytes - earlier.ws_bytes,
            d8_calls: self.d8_calls - earlier.d8_calls,
            d8_hits: self.d8_hits - earlier.d8_hits,
        }
    }
}

/// Whether this build carries the counters, so a caller cannot read a zero as
/// "no work happened" when it means "nothing was counted".
#[must_use]
pub const fn enabled() -> bool {
    cfg!(feature = "profile")
}

/// Add to a counter. Compiles to nothing without `profile`.
#[inline(always)]
pub fn add(counter: &AtomicU64, by: u64) {
    #[cfg(feature = "profile")]
    counter.fetch_add(by, Ordering::Relaxed);
    #[cfg(not(feature = "profile"))]
    {
        let _ = (counter, by);
    }
}
