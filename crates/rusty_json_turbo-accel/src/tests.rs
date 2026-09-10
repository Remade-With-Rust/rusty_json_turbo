//! Every kernel against the scalar oracle, over every byte value at every
//! offset.
//!
//! The gate a vector kernel has to clear is not "it works on the corpus". It
//! is that it agrees with the oracle on inputs the corpus does not contain --
//! a lone byte at offset 15 of a 16-byte step, a run that ends exactly on a
//! step boundary, a `0x0B` that looks like whitespace to a careless range
//! test, a `0x80` that looks negative to a signed compare.

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use crate::{escape, whitespace};

/// Buffers that put every byte value at every offset around each step
/// boundary, plus runs that end exactly on one.
fn buffers(filler: u8) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    // Every byte value, alone, at every offset in buffers spanning the 8, 16
    // and 32-byte boundaries and their neighbourhoods.
    for b in 0..=255u8 {
        for len in [1usize, 7, 8, 9, 15, 16, 17, 31, 32, 33, 47, 48, 63, 64, 65] {
            for pos in 0..len {
                let mut v = vec![filler; len];
                v[pos] = b;
                out.push(v);
            }
        }
    }
    // Uniform runs of every length across all three boundaries.
    for len in 0..70usize {
        out.push(vec![filler; len]);
        out.push(vec![b'x'; len]);
        out.push(vec![b' '; len]);
        out.push(vec![b'"'; len]);
        out.push(vec![0x00; len]);
        out.push(vec![0x1F; len]);
        out.push(vec![0x20; len]);
        out.push(vec![0x0B; len]); // vertical tab: NOT JSON whitespace
        out.push(vec![0x0C; len]); // form feed: NOT JSON whitespace
        out.push(vec![0x80; len]); // looks negative to a signed compare
        out.push(vec![0xFF; len]);
    }
    // Deterministic mixed content over the interesting alphabet.
    let alphabet = [
        b' ', b'\n', b'\t', b'\r', b'x', b'"', 0x5C, 0x00, 0x1F, 0x20, 0x0B, 0x0C, 0x80, 0xFF,
    ];
    let mut s = 0x2545_F491_4F6C_DD1Du64;
    for len in [1usize, 8, 16, 17, 32, 33, 64, 70, 129, 200] {
        for _ in 0..120 {
            let mut v = Vec::with_capacity(len);
            for _ in 0..len {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                v.push(alphabet[(s % alphabet.len() as u64) as usize]);
            }
            out.push(v);
        }
    }
    out.push(Vec::new());
    out
}

#[test]
fn whitespace_kernels_match_the_oracle() {
    let mut checked = 0u64;
    for buf in buffers(b' ') {
        for from in 0..=buf.len() {
            let want = whitespace::first_non_ws_scalar(&buf, from);
            for (name, kernel) in whitespace::all_kernels() {
                assert_eq!(
                    kernel(&buf, from),
                    want,
                    "{name} disagreed on {buf:?} from {from}"
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 500_000, "only {checked} cases -- spread too thin");
}

#[test]
fn escape_kernels_match_the_oracle() {
    let mut checked = 0u64;
    for buf in buffers(b'x') {
        for from in 0..=buf.len() {
            let want = escape::first_escape_scalar(&buf, from);
            for (name, kernel) in escape::all_kernels() {
                assert_eq!(
                    kernel(&buf, from),
                    want,
                    "{name} disagreed on {buf:?} from {from}"
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 500_000, "only {checked} cases -- spread too thin");
}

/// The two predicates, asserted over all 256 values rather than argued.
#[test]
fn predicates_are_exact() {
    for b in 0..=255u8 {
        assert_eq!(
            whitespace::is_ws(b),
            b == b' ' || b == b'\n' || b == b'\t' || b == b'\r',
            "whitespace predicate wrong for {b:#04x}"
        );
        assert_eq!(
            escape::needs_escape(b),
            b < 0x20 || b == b'"' || b == 0x5C,
            "escape predicate wrong for {b:#04x}"
        );
        // The masked form must be exactly the range test, which is what lets
        // the vector kernels avoid a signed comparison.
        assert_eq!(b & 0xE0 == 0, b < 0x20, "0xE0 mask wrong for {b:#04x}");
    }
    // The two bytes a careless "is it whitespace" range test would swallow.
    assert!(!whitespace::is_ws(0x0B));
    assert!(!whitespace::is_ws(0x0C));
    // Nothing at or above 0x80 may ever be flagged for escaping: doing so
    // would split a UTF-8 sequence.
    for b in 0x80..=0xFFu8 {
        assert!(!escape::needs_escape(b), "non-ASCII {b:#04x} flagged");
    }
}

/// POISON THE GATE. A suite that cannot fail is not a gate, so this asserts
/// the twin tests above would actually catch the two mistakes most likely to
/// be made here: a signed comparison for `b < 0x20`, and a range test for
/// whitespace that swallows the vertical tab and form feed.
#[test]
fn the_twin_tests_would_catch_the_classic_mistakes() {
    // A signed `b < 0x20` flags every byte from 0x80 up.
    fn signed_ctrl_test(b: u8) -> bool {
        (b as i8) < 0x20 || b == b'"' || b == 0x5C
    }
    let signed_disagreements = (0..=255u8)
        .filter(|&b| signed_ctrl_test(b) != escape::needs_escape(b))
        .count();
    assert!(
        signed_disagreements > 0,
        "a signed control-character test agreed with the correct one everywhere \
         -- the escape gate has stopped discriminating"
    );

    // A range test `b <= 0x20` swallows the vertical tab and form feed.
    fn range_ws(b: u8) -> bool {
        b <= b' '
    }
    let ws_disagreements = (0..=255u8)
        .filter(|&b| range_ws(b) != whitespace::is_ws(b))
        .count();
    assert!(
        ws_disagreements > 0,
        "a range whitespace test agreed with the correct one everywhere -- the \
         whitespace gate has stopped discriminating"
    );
}

/// The dispatcher must never claim an ISA the machine cannot run, and
/// `RJT_ISA` must only ever narrow.
#[test]
fn dispatch_never_widens_past_the_machine() {
    let available = crate::isa_available();
    let chosen = crate::isa();
    assert!(
        chosen.step() <= available.step(),
        "dispatch chose {} on a machine that offers only {}",
        chosen.name(),
        available.name()
    );
}
