//! Find the first byte that is not JSON insignificant whitespace.
//!
//! JSON whitespace is exactly four bytes: space, newline, tab, carriage
//! return. Everything here finds the first byte at or after `from` that is
//! none of them, returning `slice.len()` when the run reaches the end.

use crate::Isa;

/// Is this byte JSON insignificant whitespace?
///
/// Exactly four values, per RFC 8259. Notably NOT the vertical tab (`0x0B`) or
/// form feed (`0x0C`), which a range test would wrongly include -- a mistake
/// this project's whitespace tests specifically probe for.
#[must_use]
#[inline(always)]
pub fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\t' | b'\r')
}

/// THE ORACLE. One byte at a time, obviously correct, and permanent.
///
/// Every kernel below is tested against this, and `RJT_ISA=scalar` runs it in
/// production so the two can be A/B'd inside one binary.
#[must_use]
pub fn first_non_ws_scalar(slice: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < slice.len() && is_ws(slice[i]) {
        i += 1;
    }
    i
}

// ---------------------------------------------------------------- SWAR (8 B)

const ONES: u64 = 0x0101_0101_0101_0101;
const HIGHS: u64 = 0x8080_8080_8080_8080;

/// High bit set in each lane that is zero.
///
/// EXACT. The famous `(x - ONES) & !x & HIGHS` borrows across lane boundaries
/// and reports a byte as zero because its NEIGHBOUR was; masking the high bits
/// off before the add stops the carry crossing.
#[inline(always)]
fn zero_bytes(x: u64) -> u64 {
    !(((x & !HIGHS).wrapping_add(!HIGHS)) | x) & HIGHS
}

#[inline(always)]
fn eq_bytes(x: u64, c: u8) -> u64 {
    zero_bytes(x ^ (c as u64).wrapping_mul(ONES))
}

/// High bit set in each lane that is NOT whitespace.
#[inline(always)]
fn non_ws_mask_swar(x: u64) -> u64 {
    let ws = eq_bytes(x, b' ') | eq_bytes(x, b'\n') | eq_bytes(x, b'\t') | eq_bytes(x, b'\r');
    !ws & HIGHS
}

fn first_non_ws_swar(slice: &[u8], from: usize) -> usize {
    let mut i = from;
    while let Some(window) = slice.get(i..i + 8) {
        let Ok(eight) = <[u8; 8]>::try_from(window) else {
            break;
        };
        let m = non_ws_mask_swar(u64::from_le_bytes(eight));
        if m != 0 {
            return i + (m.trailing_zeros() >> 3) as usize;
        }
        i += 8;
    }
    first_non_ws_scalar(slice, i)
}

// ------------------------------------------------------------- SSE2 (16 B)

// `unsafe fn`, not a safe fn with `#[target_feature]`. Applying that attribute
// to a SAFE function is the `target_feature_11` feature, stable only from Rust
// 1.86, and this workspace's MSRV is 1.85 -- so the pre-1.86 spelling is the
// portable one. The caller's obligation is unchanged and stated at each call
// site: the target feature must be present.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn first_non_ws_sse2(slice: &[u8], from: usize) -> usize {
    use core::arch::x86_64::{
        _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_or_si128, _mm_set1_epi8,
    };

    let mut i = from;
    // SAFETY: the loop condition guarantees `i + 16 <= slice.len()`, so the 16
    // bytes read start inside the slice and end at or before its end.
    // `_mm_loadu_si128` is the UNALIGNED load, so no alignment obligation is
    // taken on. Every other intrinsic here is a register operation on an
    // already-loaded value and touches no memory. `sse2` is baseline on this
    // target, so the target feature cannot be missing.
    unsafe {
        let sp = _mm_set1_epi8(b' ' as i8);
        let nl = _mm_set1_epi8(b'\n' as i8);
        let tab = _mm_set1_epi8(b'\t' as i8);
        let cr = _mm_set1_epi8(b'\r' as i8);
        while i + 16 <= slice.len() {
            let v = _mm_loadu_si128(slice.as_ptr().add(i).cast());
            let ws = _mm_or_si128(
                _mm_or_si128(_mm_cmpeq_epi8(v, sp), _mm_cmpeq_epi8(v, nl)),
                _mm_or_si128(_mm_cmpeq_epi8(v, tab), _mm_cmpeq_epi8(v, cr)),
            );
            // A set bit means whitespace, so the first CLEAR bit is the
            // answer. Restricted to 16 bits before inverting, or the upper
            // bits of the `i32` would read as an immediate hit.
            let not_ws = !(_mm_movemask_epi8(ws) as u32) & 0xFFFF;
            if not_ws != 0 {
                return i + not_ws.trailing_zeros() as usize;
            }
            i += 16;
        }
    }
    first_non_ws_swar(slice, i)
}

// ------------------------------------------------------------- AVX2 (32 B)

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn first_non_ws_avx2(slice: &[u8], from: usize) -> usize {
    use core::arch::x86_64::{
        _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8, _mm256_or_si256,
        _mm256_set1_epi8,
    };

    let mut i = from;
    // SAFETY: as the SSE2 kernel -- the loop condition guarantees
    // `i + 32 <= slice.len()`, the load is the unaligned form, and the rest is
    // register arithmetic. The `avx2` target feature is guaranteed by the
    // caller, which only reaches here after `is_x86_feature_detected!`.
    unsafe {
        let sp = _mm256_set1_epi8(b' ' as i8);
        let nl = _mm256_set1_epi8(b'\n' as i8);
        let tab = _mm256_set1_epi8(b'\t' as i8);
        let cr = _mm256_set1_epi8(b'\r' as i8);
        while i + 32 <= slice.len() {
            let v = _mm256_loadu_si256(slice.as_ptr().add(i).cast());
            let ws = _mm256_or_si256(
                _mm256_or_si256(_mm256_cmpeq_epi8(v, sp), _mm256_cmpeq_epi8(v, nl)),
                _mm256_or_si256(_mm256_cmpeq_epi8(v, tab), _mm256_cmpeq_epi8(v, cr)),
            );
            let not_ws = !(_mm256_movemask_epi8(ws) as u32);
            if not_ws != 0 {
                return i + not_ws.trailing_zeros() as usize;
            }
            i += 32;
        }
    }
    // Deliberately hands the tail to SSE2 rather than to the scalar walk: a
    // 31-byte remainder is still two 16-byte steps.
    first_non_ws_sse2_entry(slice, i)
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn first_non_ws_sse2_entry(slice: &[u8], from: usize) -> usize {
    // SAFETY: SSE2 is baseline on x86-64, guaranteed by the target itself, so
    // this call needs no detection and cannot reach an unsupported CPU.
    unsafe { first_non_ws_sse2(slice, from) }
}

// AN ESCALATION GATE WAS TRIED HERE AND MEASURED WORSE. The idea was to take
// one inline SWAR step before calling any vector kernel, on the theory that a
// `#[target_feature]` function cannot be inlined and so pays call and setup
// cost that a two-byte run cannot repay.
//
// It cost `citm_catalog` half its win -- scan fell from 1.099x to 1.046x and
// struct-parse from 1.072x to 1.016x -- and did NOT recover `twitter`, which
// stayed at 0.978x. So the theory was wrong: the loss on `twitter` is not the
// vector call's setup.
//
// Reading the run lengths says why the gate could not have worked. After the
// parser's own 4-byte peel, `twitter`'s 12,118 runs of 9-16 bytes have 5 to 12
// bytes left, so an 8-byte SWAR step often does NOT finish them and the vector
// call happens anyway -- the gate added a step without avoiding anything. And
// `citm_catalog`'s runs of 17-32 have 13 to 28 left, which one 32-byte step
// finishes outright, so for it the gate was pure added cost.

/// First byte at or after `from` that is not JSON whitespace.
///
/// Dispatches on [`crate::isa`], which honours `RJT_ISA`.
#[must_use]
#[inline]
pub fn first_non_ws(slice: &[u8], from: usize) -> usize {
    match crate::isa() {
        Isa::Scalar => first_non_ws_scalar(slice, from),
        Isa::Swar => first_non_ws_swar(slice, from),
        #[cfg(target_arch = "x86_64")]
        Isa::Sse2 => first_non_ws_sse2_entry(slice, from),
        #[cfg(target_arch = "x86_64")]
        Isa::Avx2 => {
            // SAFETY: `isa()` returns `Avx2` only after
            // `is_x86_feature_detected!("avx2")` said so, and it never widens
            // past what that reported.
            unsafe { first_non_ws_avx2(slice, from) }
        }
        #[cfg(not(target_arch = "x86_64"))]
        _ => first_non_ws_swar(slice, from),
    }
}

/// Every kernel, for the twin tests. Not public API.
#[cfg(test)]
pub(crate) fn all_kernels() -> &'static [(&'static str, crate::Scanner)] {
    #[cfg(target_arch = "x86_64")]
    {
        &[
            ("swar", first_non_ws_swar),
            ("sse2", first_non_ws_sse2_entry),
            ("avx2", avx2_if_available),
            // The DISPATCHER, not just the kernels: the escalation gate hands
            // the wide path `from + 8`, and an off-by-one there would be
            // invisible to a test that only exercised the kernels directly.
            ("dispatch", first_non_ws),
        ]
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        &[("swar", first_non_ws_swar), ("dispatch", first_non_ws)]
    }
}

/// AVX2 when the machine has it, and the oracle when it does not, so the twin
/// test is meaningful on a CPU without AVX2 instead of silently skipped.
#[cfg(all(test, target_arch = "x86_64"))]
fn avx2_if_available(slice: &[u8], from: usize) -> usize {
    if crate::isa_available() == Isa::Avx2 {
        // SAFETY: guarded by the detection immediately above.
        unsafe { first_non_ws_avx2(slice, from) }
    } else {
        first_non_ws_scalar(slice, from)
    }
}
