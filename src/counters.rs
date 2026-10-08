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

use core::sync::atomic::Ordering;

/// The counter's width, chosen by what the target can actually do.
///
/// `AtomicU64` does not exist on a 32-bit target without 64-bit atomics --
/// riscv32imac and riscv32imafc are two -- and this module is declared
/// unconditionally, so importing it there failed the build outright
/// (`unresolved import core::sync::atomic::AtomicU64`). It broke every
/// no_std consumer of this crate on those chips.
///
/// Nothing is lost by narrowing: `add` only increments under `profile`,
/// which pulls in `std`, so on a no_std target these are never written at
/// all -- they only have to EXIST. Where 64-bit atomics are available the
/// width is unchanged.
#[cfg(target_has_atomic = "64")]
pub type Counter = core::sync::atomic::AtomicU64;
#[cfg(target_has_atomic = "64")]
type Width = u64;
/// A counter, 32 bits wide because the target has no 64-bit atomics.
#[cfg(not(target_has_atomic = "64"))]
pub type Counter = core::sync::atomic::AtomicU32;
#[cfg(not(target_has_atomic = "64"))]
type Width = u32;

/// One counter's value, as the reported 64-bit type.
///
/// Split per width so that neither arm converts a type to itself: clippy's
/// `useless_conversion` and `unnecessary_cast` each object to one of the
/// single-expression spellings.
#[cfg(target_has_atomic = "64")]
#[inline]
fn get(counter: &Counter) -> u64 {
    counter.load(Ordering::Relaxed)
}

/// One counter's value, widened from the narrow counter this target uses.
#[cfg(not(target_has_atomic = "64"))]
#[inline]
fn get(counter: &Counter) -> u64 {
    u64::from(counter.load(Ordering::Relaxed))
}

/// Calls to `Read::peek`.
pub static PEEK: Counter = Counter::new(0);
/// Calls to `Read::next`.
pub static NEXT: Counter = Counter::new(0);
/// Calls to `Read::discard`.
pub static DISCARD: Counter = Counter::new(0);
/// Calls to `Read::skip_whitespace` -- whitespace runs considered.
pub static WS_RUNS: Counter = Counter::new(0);
/// Insignificant whitespace bytes skipped.
pub static WS_BYTES: Counter = Counter::new(0);
/// Calls to `Read::take_8_digits`.
pub static D8_CALLS: Counter = Counter::new(0);
/// Calls to `Read::take_8_digits` that consumed eight digits.
pub static D8_HITS: Counter = Counter::new(0);
/// Calls to `SliceRead::skip_to_escape` -- string runs considered (brick B2).
pub static STR_SCANS: Counter = Counter::new(0);
/// String content bytes advanced over by the escape scanner (brick B2).
pub static STR_BYTES: Counter = Counter::new(0);
/// Strings that had to be copied into scratch because they held an escape, so
/// the zero-copy borrow was lost (brick B2: this is the cost worth removing).
pub static STR_COPIES: Counter = Counter::new(0);
/// Strings returned as a borrow of the input, the fast path (brick B2).
pub static STR_BORROWS: Counter = Counter::new(0);
/// String content bytes examined by the serializer's escape loop (brick B3).
pub static ESC_BYTES: Counter = Counter::new(0);
/// Bytes that actually needed a JSON escape (brick B3). The ratio of this to
/// `ESC_BYTES` is what a per-chunk mask would be skipping over.
pub static ESC_HITS: Counter = Counter::new(0);
/// `write_string_fragment` calls: clean runs handed to the writer (brick B3).
pub static ESC_FRAGS: Counter = Counter::new(0);
/// Steps taken by the escape scanner: one per 8-byte chunk on the wide path,
/// one per byte on the scalar path (brick B3). This is the counter that says
/// whether the wide path is ENGAGED -- a byte-identical output gate cannot,
/// because a fast path that silently stopped being taken still produces the
/// right answer. Expect roughly `esc_bytes / 8` wide and `esc_bytes` scalar.
pub static ESC_STEPS: Counter = Counter::new(0);
/// Object keys parsed (brick B6). One per key, whatever the visitor does next.
pub static KEYS: Counter = Counter::new(0);
/// `RawValue` captures completed on a reader (brick B13).
pub static RAW_CAPTURES: Counter = Counter::new(0);
/// Bytes of `RawValue` text captured on a reader (brick B13). Upstream pushed
/// one byte at a time and this is how many; B13 takes the same total in
/// `RAW_CAPTURES` copies, so the ratio of the two is the brick's whole effect.
pub static RAW_BYTES: Counter = Counter::new(0);
/// Times the window had to GROW because a captured value was longer than it
/// (brick B13). Zero on any stream of ordinary documents; non-zero is the only
/// case where B13 costs memory that upstream did not spend.
pub static RAW_GROWS: Counter = Counter::new(0);
/// Times a grown window was handed back at the end of a capture (brick B13).
/// This is the counter that says the growth is not permanent -- there is no
/// output difference to gate it with.
pub static RAW_SHRINKS: Counter = Counter::new(0);
/// Bytes handed to `str::from_utf8` for validation (brick B12). `from_str`
/// pays none of this because its input is already known to be UTF-8, so the
/// gap between `from_slice` and `from_str` on the same bytes is B12's ceiling.
pub static UTF8_BYTES: Counter = Counter::new(0);
/// Calls to that validation (brick B12).
pub static UTF8_CALLS: Counter = Counter::new(0);

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
        peek: get(&PEEK),
        next: get(&NEXT),
        discard: get(&DISCARD),
        ws_runs: get(&WS_RUNS),
        ws_bytes: get(&WS_BYTES),
        d8_calls: get(&D8_CALLS),
        d8_hits: get(&D8_HITS),
        str_scans: get(&STR_SCANS),
        str_bytes: get(&STR_BYTES),
        str_copies: get(&STR_COPIES),
        str_borrows: get(&STR_BORROWS),
        esc_bytes: get(&ESC_BYTES),
        esc_hits: get(&ESC_HITS),
        esc_frags: get(&ESC_FRAGS),
        esc_steps: get(&ESC_STEPS),
        keys: get(&KEYS),
        raw_captures: get(&RAW_CAPTURES),
        raw_bytes: get(&RAW_BYTES),
        raw_grows: get(&RAW_GROWS),
        raw_shrinks: get(&RAW_SHRINKS),
        utf8_bytes: get(&UTF8_BYTES),
        utf8_calls: get(&UTF8_CALLS),
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

/// Whether the optional SIMD island is linked (the `accel` feature).
///
/// [`isa`] cannot answer this. Off `x86_64` the island's own answer is
/// `"swar"`, which is exactly what the in-crate SWAR reports, so the two
/// configurations are indistinguishable by ISA name alone.
///
/// **It has to be distinguishable, because one counter means two things.**
/// [`ESC_STEPS`] is incremented by this crate's escape scanner once per 8-byte
/// chunk, and by the island once per CALL -- the island is a separate `no_std`
/// crate with no access to these statics, so it cannot count chunks even in
/// principle. Measured on `twitter`, the same document reads 101,382 steps
/// without the island and 19,327 with it: about one per string rather than one
/// per chunk. A census that did not know which build it was looking at would
/// read the second as a nineteen-byte step and conclude something impossible.
///
/// Found by `tests/m6_arch_census.rs` failing on exactly that.
#[must_use]
pub const fn accel_linked() -> bool {
    cfg!(feature = "accel")
}

/// Add to a counter. Compiles to nothing without `profile`.
#[inline(always)]
pub fn add(counter: &Counter, by: u64) {
    #[cfg(all(feature = "profile", target_has_atomic = "64"))]
    counter.fetch_add(by, Ordering::Relaxed);
    // saturating rather than wrapping: on a narrow counter a wrap would be a
    // silently wrong number, and a counter is only ever read as a total
    #[cfg(all(feature = "profile", not(target_has_atomic = "64")))]
    counter.fetch_add(Width::try_from(by).unwrap_or(Width::MAX), Ordering::Relaxed);
    #[cfg(not(feature = "profile"))]
    {
        let _ = (counter, by);
    }
}
