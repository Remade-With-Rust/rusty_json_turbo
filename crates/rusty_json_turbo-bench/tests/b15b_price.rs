//! CEILING PROBE for B15's surviving half, before anything is built.
//!
//! B15 folded a short escape-free string's quotes and contents into ONE
//! `write_all` and ran 11-15% SLOWER, because `write_all(b"\"")` on a `Vec` is
//! `extend_from_slice` with a COMPILE-TIME length -- a capacity check and a
//! store -- and folding traded two of those for a runtime-length copy in and a
//! runtime-length copy out. The ledger recorded that one half of the idea
//! survives that reasoning: folding the **constant separators** together,
//! `,` + `"` into `,"` and `"` + `:` into `":`, keeps both lengths constant.
//! Two capacity checks become one; no `memcpy` appears anywhere.
//!
//! It also recorded that it prices at "well under 1%", which is below this
//! project's measurement floor -- so it was to be batched with other sub-1%
//! items and measured together. This probe is what says whether that batch is
//! worth assembling, and it is deliberately BIASED IN THE BRICK'S FAVOUR:
//!
//! - the writes happen in a tight loop with nothing else competing for the
//!   store ports, perfect branch prediction and a hot L1,
//! - the sink is a `Vec<u8>` with capacity already reserved, so no growth,
//! - and the real serializer would additionally have to prove the key needs no
//!   escaping before it could fold anything.
//!
//! So whatever this measures is an UPPER BOUND on the brick. If the upper
//! bound is under the floor, the brick is refuted without being built --
//! ceiling probes before bricks.
//!
//! # WHAT HAPPENED NEXT, AND READ THIS BEFORE TRUSTING THE NUMBER
//!
//! This probe read **2.92% and 3.01%**, above the floor, so B15s was built. It
//! was then measured against the whole program by a knob A/B -- 41 pairs on a
//! quiet box, a null arm, two controls -- and delivered **0.0%: 0.996x median
//! and 0.990x best-of-N on `s4-frame-telemetry`, where 46,800 of 46,816 keys
//! are struct fields.** Both statistics said SLOWER. B15s was reverted.
//!
//! The correction to the reasoning above is the finding, and it is why this
//! file is kept:
//!
//! > **An isolated micro-probe bounds the WORK a brick removes, not the TIME
//! > the program will save.** The surrounding program may already be hiding
//! > that work -- under other latency, or because the compiler already merged
//! > it.
//! >
//! > **So a tight-loop ceiling probe can REFUTE a brick. It can never JUSTIFY
//! > one.** Only a knob A/B against the whole program can do that.
//!
//! Reported, not asserted, except for the work-parity check: both arms must
//! produce byte-identical output, or they are not two ways of doing one thing.

use std::io::Write;
use std::time::Instant;

fn best_of<F: FnMut() -> u64>(mut f: F, n: usize) -> u64 {
    let mut best = u64::MAX;
    for _ in 0..n {
        best = best.min(f());
    }
    best
}

/// Every object key in a document, in the order the serializer would write
/// them -- the real distribution, not a synthetic one. `citm_catalog` is the
/// key-heaviest cell in the corpus at 25,869.
fn keys_of(file: rjt_bench::corpus::File) -> Vec<String> {
    let bytes = file.load();
    let v: turbo::Value = turbo::from_slice(&bytes).expect("corpus member parses");
    let mut out = Vec::new();
    fn walk(v: &turbo::Value, out: &mut Vec<String>) {
        match v {
            turbo::Value::Object(m) => {
                for (k, val) in m {
                    out.push(k.clone());
                    walk(val, out);
                }
            }
            turbo::Value::Array(a) => {
                for val in a {
                    walk(val, out);
                }
            }
            _ => {}
        }
    }
    walk(&v, &mut out);
    out
}

/// Upstream's shape: the separator, the quote, the contents, the quote and the
/// colon, as five calls with compile-time lengths of 1, 1, n, 1, 1.
#[inline(never)]
fn unfolded(keys: &[String], sink: &mut Vec<u8>) {
    sink.clear();
    for (i, k) in keys.iter().enumerate() {
        if i > 0 {
            sink.write_all(b",").unwrap();
        }
        sink.write_all(b"\"").unwrap();
        sink.write_all(k.as_bytes()).unwrap();
        sink.write_all(b"\"").unwrap();
        sink.write_all(b":").unwrap();
    }
}

/// The brick: three calls, with compile-time lengths of 2, n, 2. Nothing
/// became runtime-length, which is the whole reason this half of B15 survived
/// the reasoning that killed the other half.
#[inline(never)]
fn folded(keys: &[String], sink: &mut Vec<u8>) {
    sink.clear();
    for (i, k) in keys.iter().enumerate() {
        if i > 0 {
            sink.write_all(b",\"").unwrap();
        } else {
            sink.write_all(b"\"").unwrap();
        }
        sink.write_all(k.as_bytes()).unwrap();
        sink.write_all(b"\":").unwrap();
    }
}

#[test]
// A timing probe in a debug build measures the debug build, so it is skipped
// in the default `cargo test` run and asked for explicitly in release.
#[cfg_attr(debug_assertions, ignore = "release only: this is a timing probe")]
fn price_the_constant_separator_fold_against_the_stringify_it_would_speed_up() {
    for file in [
        rjt_bench::corpus::File::CitmCatalog,
        rjt_bench::corpus::File::Twitter,
    ] {
        let keys = keys_of(file);
        let bytes = file.load();
        let value: turbo::Value = turbo::from_slice(&bytes).expect("parses");

        // WORK PARITY, asserted before timing: two ways of writing one thing.
        let mut a = Vec::with_capacity(1 << 20);
        let mut b = Vec::with_capacity(1 << 20);
        unfolded(&keys, &mut a);
        folded(&keys, &mut b);
        assert_eq!(a, b, "{}: the two arms write different bytes", file.name());

        let mut sink = Vec::with_capacity(a.len() + 64);
        let time = |f: fn(&[String], &mut Vec<u8>), sink: &mut Vec<u8>| -> u64 {
            best_of(
                || {
                    let t = Instant::now();
                    f(&keys, sink);
                    let ns = t.elapsed().as_nanos() as u64;
                    std::hint::black_box(sink.len());
                    ns
                },
                15,
            )
        };
        // Alternate, so neither arm is always the cold one.
        let mut un = u64::MAX;
        let mut fo = u64::MAX;
        for _ in 0..5 {
            un = un.min(time(unfolded, &mut sink));
            fo = fo.min(time(folded, &mut sink));
            fo = fo.min(time(folded, &mut sink));
            un = un.min(time(unfolded, &mut sink));
        }

        // The denominator: the stringify the brick would be speeding up.
        let whole = best_of(
            || {
                let t = Instant::now();
                let s = turbo::to_string(&value).expect("stringify");
                let ns = t.elapsed().as_nanos() as u64;
                std::hint::black_box(s.len());
                ns
            },
            15,
        );

        let saved = un.saturating_sub(fo);
        println!(
            "\n{}: {} keys\n  \
             five calls per key   {:>9} ns\n  \
             three calls per key  {:>9} ns\n  \
             saved                {:>9} ns  ({:.2} ns/key)\n  \
             whole dom-stringify  {:>9} ns\n  \
             => CEILING on the constant-separator fold: {:.3}% of stringify \
             (upper bound: tight loop, reserved sink, no escape check)",
            file.name(),
            keys.len(),
            un,
            fo,
            saved,
            saved as f64 / keys.len() as f64,
            whole,
            saved as f64 / whole as f64 * 100.0,
        );
    }
}
