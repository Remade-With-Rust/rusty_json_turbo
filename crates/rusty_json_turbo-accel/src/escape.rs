//! Find the first byte in a JSON string that must be escaped on output.
//!
//! The serializer's escape table is nonzero for exactly three things:
//! `0x00..=0x1F`, `"` and `\`. So the predicate is
//! `b < 0x20 || b == 0x22 || b == 0x5C`.
//!
//! `b < 0x20` is expressed throughout as `b & 0xE0 == 0`, which is EXACT --
//! `0x20` is a single bit, so every value below it has the top three bits
//! clear. That formulation matters for the vector kernels specifically: SSE2
//! and AVX2 byte comparisons are SIGNED, so a naive `cmplt` against `0x20`
//! would treat every byte from `0x80` up as negative and flag it, splitting
//! multi-byte UTF-8 sequences. An equality test against a masked value has no
//! signedness at all.

use crate::Isa;

/// Does this byte need a JSON escape?
#[must_use]
#[inline(always)]
pub fn needs_escape(b: u8) -> bool {
    b & 0xE0 == 0 || b == b'"' || b == 0x5C
}

/// THE ORACLE. One byte at a time, and permanent.
#[must_use]
pub fn first_escape_scalar(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < bytes.len() && !needs_escape(bytes[i]) {
        i += 1;
    }
    i
}

// ---------------------------------------------------------------- SWAR (8 B)

const ONES: u64 = 0x0101_0101_0101_0101;
const HIGHS: u64 = 0x8080_8080_8080_8080;

#[inline(always)]
fn zero_bytes(x: u64) -> u64 {
    !(((x & !HIGHS).wrapping_add(!HIGHS)) | x) & HIGHS
}

#[inline(always)]
fn eq_bytes(x: u64, c: u8) -> u64 {
    zero_bytes(x ^ (c as u64).wrapping_mul(ONES))
}

#[inline(always)]
fn escape_mask_swar(x: u64) -> u64 {
    // `b & 0xE0 == 0` is `b < 0x20`, exactly.
    zero_bytes(x & 0xE0u64.wrapping_mul(ONES)) | eq_bytes(x, b'"') | eq_bytes(x, 0x5C)
}

fn first_escape_swar(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while let Some(window) = bytes.get(i..i + 8) {
        let Ok(eight) = <[u8; 8]>::try_from(window) else {
            break;
        };
        let m = escape_mask_swar(u64::from_le_bytes(eight));
        if m != 0 {
            return i + (m.trailing_zeros() >> 3) as usize;
        }
        i += 8;
    }
    first_escape_scalar(bytes, i)
}

// ------------------------------------------------------------- SSE2 (16 B)

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn first_escape_sse2(bytes: &[u8], from: usize) -> usize {
    use core::arch::x86_64::{
        _mm_and_si128, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_or_si128,
        _mm_set1_epi8, _mm_setzero_si128,
    };

    let mut i = from;
    // SAFETY: the loop condition guarantees `i + 16 <= bytes.len()`, so the
    // read starts inside the slice and ends at or before its end;
    // `_mm_loadu_si128` is the unaligned load and takes on no alignment
    // obligation. Everything else is register arithmetic on loaded values.
    unsafe {
        let hi3 = _mm_set1_epi8(0xE0u8 as i8);
        let zero = _mm_setzero_si128();
        let quote = _mm_set1_epi8(b'"' as i8);
        let backslash = _mm_set1_epi8(0x5C);
        while i + 16 <= bytes.len() {
            let v = _mm_loadu_si128(bytes.as_ptr().add(i).cast());
            // `(v & 0xE0) == 0` is `v < 0x20` with no signed comparison.
            let ctrl = _mm_cmpeq_epi8(_mm_and_si128(v, hi3), zero);
            let m = _mm_or_si128(
                ctrl,
                _mm_or_si128(_mm_cmpeq_epi8(v, quote), _mm_cmpeq_epi8(v, backslash)),
            );
            let hits = _mm_movemask_epi8(m) as u32 & 0xFFFF;
            if hits != 0 {
                return i + hits.trailing_zeros() as usize;
            }
            i += 16;
        }
    }
    first_escape_swar(bytes, i)
}

// ------------------------------------------------------------- AVX2 (32 B)

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn first_escape_avx2(bytes: &[u8], from: usize) -> usize {
    use core::arch::x86_64::{
        _mm256_and_si256, _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8,
        _mm256_or_si256, _mm256_set1_epi8, _mm256_setzero_si256,
    };

    let mut i = from;
    // SAFETY: as SSE2 above -- bounds come from the loop condition, the load
    // is unaligned, and `avx2` is guaranteed by the caller's detection.
    unsafe {
        let hi3 = _mm256_set1_epi8(0xE0u8 as i8);
        let zero = _mm256_setzero_si256();
        let quote = _mm256_set1_epi8(b'"' as i8);
        let backslash = _mm256_set1_epi8(0x5C);
        while i + 32 <= bytes.len() {
            let v = _mm256_loadu_si256(bytes.as_ptr().add(i).cast());
            let ctrl = _mm256_cmpeq_epi8(_mm256_and_si256(v, hi3), zero);
            let m = _mm256_or_si256(
                ctrl,
                _mm256_or_si256(_mm256_cmpeq_epi8(v, quote), _mm256_cmpeq_epi8(v, backslash)),
            );
            let hits = _mm256_movemask_epi8(m) as u32;
            if hits != 0 {
                return i + hits.trailing_zeros() as usize;
            }
            i += 32;
        }
    }
    first_escape_sse2_entry(bytes, i)
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn first_escape_sse2_entry(bytes: &[u8], from: usize) -> usize {
    // SAFETY: SSE2 is baseline on x86-64, guaranteed by the target itself.
    unsafe { first_escape_sse2(bytes, from) }
}

/// First byte at or after `from` that needs a JSON escape.
///
/// # The length guard, and why it is not the gate that failed for whitespace
///
/// A `#[target_feature]` function cannot be inlined into a caller that lacks
/// the feature, so every vector call is a real call. A string shorter than one
/// vector step cannot use it -- the kernel's own loop condition fails
/// immediately and it falls back -- so that call is pure cost. And short
/// strings are the common case: the mean string is **8.3 bytes** on
/// `citm_catalog`, where the vector path measured **0.970x** on the best-of-N
/// statistic before this guard existed.
///
/// This is a LENGTH COMPARE, not an extra scan step. The escalation gate tried
/// for the whitespace scanner failed precisely because it added a SWAR step
/// that the vector call then repeated; this adds nothing to the work, it only
/// declines to make a call that provably cannot pay.
#[must_use]
#[inline]
pub fn first_escape(bytes: &[u8], from: usize) -> usize {
    let isa = crate::isa();
    if bytes.len().saturating_sub(from) < isa.step() {
        return first_escape_swar(bytes, from);
    }
    match isa {
        Isa::Scalar => first_escape_scalar(bytes, from),
        Isa::Swar => first_escape_swar(bytes, from),
        #[cfg(target_arch = "x86_64")]
        Isa::Sse2 => first_escape_sse2_entry(bytes, from),
        #[cfg(target_arch = "x86_64")]
        Isa::Avx2 => {
            // SAFETY: `isa()` returns `Avx2` only after detection said so.
            unsafe { first_escape_avx2(bytes, from) }
        }
        #[cfg(not(target_arch = "x86_64"))]
        _ => first_escape_swar(bytes, from),
    }
}

/// Every kernel, for the twin tests. Not public API.
#[cfg(test)]
pub(crate) fn all_kernels() -> &'static [(&'static str, crate::Scanner)] {
    #[cfg(target_arch = "x86_64")]
    {
        &[
            ("swar", first_escape_swar),
            ("sse2", first_escape_sse2_entry),
            ("avx2", avx2_if_available),
            ("dispatch", first_escape),
        ]
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        &[("swar", first_escape_swar), ("dispatch", first_escape)]
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
fn avx2_if_available(bytes: &[u8], from: usize) -> usize {
    if crate::isa_available() == Isa::Avx2 {
        // SAFETY: guarded by the detection immediately above.
        unsafe { first_escape_avx2(bytes, from) }
    } else {
        first_escape_scalar(bytes, from)
    }
}
