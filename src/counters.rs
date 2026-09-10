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
/// Calls to `SliceRead::skip_to_escape` -- string runs considered (brick B2).
pub static STR_SCANS: AtomicU64 = AtomicU64::new(0);
/// String content bytes advanced over by the escape scanner (brick B2).
pub static STR_BYTES: AtomicU64 = AtomicU64::new(0);
/// Strings that had to be copied into scratch because they held an escape, so
/// the zero-copy borrow was lost (brick B2: this is the cost worth removing).
pub static STR_COPIES: AtomicU64 = AtomicU64::new(0);
/// Strings returned as a borrow of the input, the fast path (brick B2).
pub static STR_BORROWS: AtomicU64 = AtomicU64::new(0);
/// String content bytes examined by the serializer's escape loop (brick B3).
pub static ESC_BYTES: AtomicU64 = AtomicU64::new(0);
/// Bytes that actually needed a JSON escape (brick B3). The ratio of this to
/// `ESC_BYTES` is what a per-chunk mask would be skipping over.
pub static ESC_HITS: AtomicU64 = AtomicU64::new(0);
/// `write_string_fragment` calls: clean runs handed to the writer (brick B3).
pub static ESC_FRAGS: AtomicU64 = AtomicU64::new(0);
/// Steps taken by the escape scanner: one per 8-byte chunk on the wide path,
/// one per byte on the scalar path (brick B3). This is the counter that says
/// whether the wide path is ENGAGED -- a byte-identical output gate cannot,
/// because a fast path that silently stopped being taken still produces the
/// right answer. Expect roughly `esc_bytes / 8` wide and `esc_bytes` scalar.
pub static ESC_STEPS: AtomicU64 = AtomicU64::new(0);
/// Object keys parsed (brick B6). One per key, whatever the visitor does next.
pub static KEYS: AtomicU64 = AtomicU64::new(0);
/// `RawValue` captures completed on a reader (brick B13).
pub static RAW_CAPTURES: AtomicU64 = AtomicU64::new(0);
/// Bytes of `RawValue` text captured on a reader (brick B13). Upstream pushed
/// one byte at a time and this is how many; B13 takes the same total in
/// `RAW_CAPTURES` copies, so the ratio of the two is the brick's whole effect.
pub static RAW_BYTES: AtomicU64 = AtomicU64::new(0);
/// Times the window had to GROW because a captured value was longer than it
/// (brick B13). Zero on any stream of ordinary documents; non-zero is the only
/// case where B13 costs memory that upstream did not spend.
pub static RAW_GROWS: AtomicU64 = AtomicU64::new(0);
/// Times a grown window was handed back at the end of a capture (brick B13).
/// This is the counter that says the growth is not permanent -- there is no
/// output difference to gate it with.
pub static RAW_SHRINKS: AtomicU64 = AtomicU64::new(0);
/// Bytes handed to `str::from_utf8` for validation (brick B12). `from_str`
/// pays none of this because its input is already known to be UTF-8, so the
/// gap between `from_slice` and `from_str` on the same bytes is B12's ceiling.
pub static UTF8_BYTES: AtomicU64 = AtomicU64::new(0);
/// Calls to that validation (brick B12).
pub static UTF8_CALLS: AtomicU64 = AtomicU64::new(0);

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
    /// String runs considered by the escape scanner.
    pub str_scans: u64,
    /// String content bytes advanced over by the escape scanner.
    pub str_bytes: u64,
    /// Strings copied into scratch because they held an escape.
    pub str_copies: u64,
    /// Strings returned as a borrow of the input.
    pub str_borrows: u64,
    /// String content bytes examined by the serializer's escape loop.
    pub esc_bytes: u64,
    /// Bytes that needed a JSON escape.
    pub esc_hits: u64,
    /// Clean runs handed to the writer.
    pub esc_frags: u64,
    /// Steps taken by the escape scanner (per chunk wide, per byte scalar).
    pub esc_steps: u64,
    /// Object keys parsed.
    pub keys: u64,
    /// `RawValue` captures completed on a reader.
    pub raw_captures: u64,
    /// Bytes of `RawValue` text captured on a reader.
    pub raw_bytes: u64,
    /// Times the window grew for a capture longer than itself.
    pub raw_grows: u64,
    /// Times a grown window was handed back.
    pub raw_shrinks: u64,
    /// Bytes handed to `str::from_utf8` for validation.
    pub utf8_bytes: u64,
    /// Calls to that validation.
    pub utf8_calls: u64,
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
        str_scans: STR_SCANS.load(Ordering::Relaxed),
        str_bytes: STR_BYTES.load(Ordering::Relaxed),
        str_copies: STR_COPIES.load(Ordering::Relaxed),
        str_borrows: STR_BORROWS.load(Ordering::Relaxed),
        esc_bytes: ESC_BYTES.load(Ordering::Relaxed),
        esc_hits: ESC_HITS.load(Ordering::Relaxed),
        esc_frags: ESC_FRAGS.load(Ordering::Relaxed),
        esc_steps: ESC_STEPS.load(Ordering::Relaxed),
        keys: KEYS.load(Ordering::Relaxed),
        raw_captures: RAW_CAPTURES.load(Ordering::Relaxed),
        raw_bytes: RAW_BYTES.load(Ordering::Relaxed),
        raw_grows: RAW_GROWS.load(Ordering::Relaxed),
        raw_shrinks: RAW_SHRINKS.load(Ordering::Relaxed),
        utf8_bytes: UTF8_BYTES.load(Ordering::Relaxed),
        utf8_calls: UTF8_CALLS.load(Ordering::Relaxed),
    }
}

/// Zero every counter.
pub fn reset() {
    for c in [
        &PEEK,
        &NEXT,
        &DISCARD,
        &WS_RUNS,
        &WS_BYTES,
        &D8_CALLS,
        &D8_HITS,
        &STR_SCANS,
        &STR_BYTES,
        &STR_COPIES,
        &STR_BORROWS,
        &ESC_BYTES,
        &ESC_HITS,
        &ESC_FRAGS,
        &ESC_STEPS,
        &KEYS,
        &RAW_CAPTURES,
        &RAW_BYTES,
        &RAW_GROWS,
        &RAW_SHRINKS,
        &UTF8_BYTES,
        &UTF8_CALLS,
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
            str_scans: self.str_scans - earlier.str_scans,
            str_bytes: self.str_bytes - earlier.str_bytes,
            str_copies: self.str_copies - earlier.str_copies,
            str_borrows: self.str_borrows - earlier.str_borrows,
            esc_bytes: self.esc_bytes - earlier.esc_bytes,
            esc_hits: self.esc_hits - earlier.esc_hits,
            esc_frags: self.esc_frags - earlier.esc_frags,
            esc_steps: self.esc_steps - earlier.esc_steps,
            keys: self.keys - earlier.keys,
            raw_captures: self.raw_captures - earlier.raw_captures,
            raw_bytes: self.raw_bytes - earlier.raw_bytes,
            raw_grows: self.raw_grows - earlier.raw_grows,
            raw_shrinks: self.raw_shrinks - earlier.raw_shrinks,
            utf8_bytes: self.utf8_bytes - earlier.utf8_bytes,
            utf8_calls: self.utf8_calls - earlier.utf8_calls,
        }
    }
}

/// Which SIMD rung the byte scanners are actually running.
///
/// Belongs on every method line that quotes a scan: `"avx2"`, `"sse2"`,
/// `"swar"` or `"scalar"`, honouring `RJT_ISA`. Returns `"swar"` when the
/// island is not linked at all, which is what the crate then does.
#[must_use]
pub fn isa() -> &'static str {
    #[cfg(feature = "accel")]
    {
        rusty_json_turbo_accel::isa().name()
    }
    #[cfg(not(feature = "accel"))]
    {
        "swar"
    }
}

/// The widest rung this machine could offer, whatever `RJT_ISA` asked for.
///
/// Printed beside [`isa`] so a run that was narrowed by an override is
/// distinguishable from one on a machine that has nothing wider -- two very
/// different reasons for the same number.
#[must_use]
pub fn isa_available() -> &'static str {
    #[cfg(feature = "accel")]
    {
        rusty_json_turbo_accel::isa_available().name()
    }
    #[cfg(not(feature = "accel"))]
    {
        "swar"
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
