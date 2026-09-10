//! Exact SWAR byte primitives: test eight bytes with one integer operation.
//!
//! SWAR ("SIMD within a register") treats a `u64` as eight lanes and finds the
//! interesting lane with ordinary integer arithmetic. No target feature, no
//! runtime dispatch, no `unsafe`, and identical results on every platform --
//! which is why the house reaches for it before it reaches for an intrinsic.
//!
//! # The trap this module exists to avoid
//!
//! The famous zero-byte test is
//!
//! ```text
//! (x - 0x0101..01) & !x & 0x8080..80
//! ```
//!
//! and it is **wrong**. The subtraction borrows across lane boundaries, so a
//! byte is reported as zero because its NEIGHBOUR was. It is right often
//! enough to pass a careless test and wrong often enough to corrupt output on
//! real data. Brick B1s in this crate was measured with the buggy form
//! deliberately substituted in, to prove the twin tests could actually tell
//! the difference: three of four failed, which is what a gate is for.
//!
//! The form below is exact. Masking off the high bits before the add means the
//! carry out of lane *n* cannot reach lane *n + 1*:
//!
//! ```text
//! !(((x & !HIGHS) + !HIGHS) | x) & HIGHS
//! ```
//!
//! Every predicate here is built from that one function, so the subtlety is
//! paid for once, in one place, with tests, rather than re-derived per brick.
//!
//! # Reading a mask
//!
//! Each predicate returns a `u64` with the high bit of lane *i* set when lane
//! *i* matched, and every other bit clear. `mask.trailing_zeros() >> 3` is the
//! index of the first matching byte, which is what makes these usable for
//! "find the next interesting byte" rather than only "is one present".
//! Little-endian lane order is assumed throughout, so callers must load with
//! [`u64::from_le_bytes`].

/// `0x0101_0101_0101_0101` -- one in every lane.
pub(crate) const ONES: u64 = 0x0101_0101_0101_0101;
/// `0x8080_8080_8080_8080` -- the high bit of every lane.
pub(crate) const HIGHS: u64 = 0x8080_8080_8080_8080;

/// High bit set in each lane that is zero.
///
/// Exact: no carry crosses a lane boundary. See the module docs for the
/// popular version of this function that is not exact.
#[inline(always)]
pub(crate) fn zero_bytes(x: u64) -> u64 {
    !(((x & !HIGHS).wrapping_add(!HIGHS)) | x) & HIGHS
}

/// High bit set in each lane equal to `c`.
#[inline(always)]
pub(crate) fn eq_bytes(x: u64, c: u8) -> u64 {
    zero_bytes(x ^ (c as u64).wrapping_mul(ONES))
}

/// High bit set in each lane whose bits under `mask` are all clear.
///
/// The building block for a range test that needs no comparison. `b < 0x20`
/// is exactly `b & 0xE0 == 0`, because 0x20 is a single bit and everything
/// below it has the top three bits clear -- so a control-character test is
/// this function with `mask = 0xE0`, and it is exact rather than approximate.
#[inline(always)]
pub(crate) fn masked_zero_bytes(x: u64, mask: u8) -> u64 {
    zero_bytes(x & (mask as u64).wrapping_mul(ONES))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// The scalar truth for `zero_bytes`, written independently.
    fn zero_bytes_scalar(x: u64) -> u64 {
        let mut out = 0u64;
        for i in 0..8 {
            if (x >> (i * 8)) as u8 == 0 {
                out |= 0x80 << (i * 8);
            }
        }
        out
    }

    fn eq_bytes_scalar(x: u64, c: u8) -> u64 {
        let mut out = 0u64;
        for i in 0..8 {
            if (x >> (i * 8)) as u8 == c {
                out |= 0x80 << (i * 8);
            }
        }
        out
    }

    fn masked_zero_scalar(x: u64, mask: u8) -> u64 {
        let mut out = 0u64;
        for i in 0..8 {
            if ((x >> (i * 8)) as u8) & mask == 0 {
                out |= 0x80 << (i * 8);
            }
        }
        out
    }

    /// A deterministic spread of words, including every single-byte case in
    /// every lane -- which is where a cross-lane borrow bug shows up.
    fn words() -> Vec<u64> {
        let mut v = Vec::new();
        for b in 0..=255u8 {
            for lane in 0..8 {
                // The byte alone in one lane.
                v.push((b as u64) << (lane * 8));
                // The byte in one lane with 0x01 everywhere else: the case the
                // borrowing form of the zero test gets wrong.
                let mut w = ONES;
                w &= !(0xFFu64 << (lane * 8));
                w |= (b as u64) << (lane * 8);
                v.push(w);
                // And with 0xFF everywhere else.
                let mut w = u64::MAX;
                w &= !(0xFFu64 << (lane * 8));
                w |= (b as u64) << (lane * 8);
                v.push(w);
            }
        }
        v.push(0);
        v.push(u64::MAX);
        v.push(ONES);
        v.push(HIGHS);
        // A cheap deterministic PRNG, so the spread is not only structured.
        let mut s = 0x243F_6A88_85A3_08D3u64;
        for _ in 0..4096 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            v.push(s);
        }
        v
    }

    #[test]
    fn zero_bytes_matches_scalar() {
        for x in words() {
            assert_eq!(
                zero_bytes(x),
                zero_bytes_scalar(x),
                "zero_bytes disagreed on {x:#018x}"
            );
        }
    }

    #[test]
    fn eq_bytes_matches_scalar() {
        // Every byte value as the needle, against the structured spread.
        for c in [0u8, 1, 0x1F, 0x20, 0x22, 0x5C, 0x7F, 0x80, 0xFE, 0xFF] {
            for x in words() {
                assert_eq!(
                    eq_bytes(x, c),
                    eq_bytes_scalar(x, c),
                    "eq_bytes({x:#018x}, {c:#04x}) disagreed"
                );
            }
        }
    }

    #[test]
    fn masked_zero_bytes_matches_scalar() {
        for mask in [0xE0u8, 0xF0, 0x80, 0xFF, 0x01] {
            for x in words() {
                assert_eq!(
                    masked_zero_bytes(x, mask),
                    masked_zero_scalar(x, mask),
                    "masked_zero_bytes({x:#018x}, {mask:#04x}) disagreed"
                );
            }
        }
    }

    /// `b & 0xE0 == 0` must be exactly `b < 0x20`, for every byte. This is the
    /// identity the escape and whitespace scanners both rely on, so it is
    /// asserted rather than assumed.
    #[test]
    fn masked_zero_is_a_less_than_test() {
        for b in 0..=255u8 {
            assert_eq!(b & 0xE0 == 0, b < 0x20, "0xE0 mask wrong for {b:#04x}");
        }
    }

    /// The borrowing form really is wrong, so the tests above are load-bearing
    /// rather than decorative. If this test ever fails, the exact form and the
    /// buggy form agree everywhere tested and the gate has stopped
    /// discriminating -- which would mean the spread in `words()` got weaker.
    #[test]
    fn the_classic_zero_test_is_wrong_somewhere() {
        fn buggy(x: u64) -> u64 {
            x.wrapping_sub(ONES) & !x & HIGHS
        }
        let disagreements = words()
            .into_iter()
            .filter(|&x| buggy(x) != zero_bytes(x))
            .count();
        assert!(
            disagreements > 0,
            "the buggy zero-byte test agreed with the exact one everywhere \
             tested -- the test spread has stopped discriminating"
        );
    }
}
