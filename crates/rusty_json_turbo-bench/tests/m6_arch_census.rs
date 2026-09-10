//! THE PER-ARCH CENSUS (G5/G6): the portable kernels are actually reached.
//!
//! The eight-target matrix proves the library COMPILES everywhere the house
//! ships. It cannot prove the fast paths are *taken* there, and that is a
//! different question with a specific failure mode: this crate's wide scanners
//! are 8-byte SWAR over `u64` loads, each guarded by a length check, and a
//! target where those guards never pass would produce **completely correct
//! output at the speed of the per-byte fallback**. No output gate can see
//! that. Only a counter can.
//!
//! So this asserts, on whatever architecture it is run on:
//!
//! - which ISA the crate believes it is running, and that it matches what the
//!   architecture can actually offer;
//! - that the whitespace scanner ran and consumed bytes;
//! - that the eight-digit number fold ran and HIT, on the file made of
//!   numbers;
//! - that the escape scanner's step count matches what the wide path PREDICTS
//!   from the document's string lengths, rather than the one-step-per-byte
//!   count the fallback would produce.
//!
//! It lives in its own binary because the counters are process-global statics
//! and a sibling test parsing in another thread would inflate every reading.
//!
//! Needs `--features count-work`. Without it the crate carries no counters and
//! this says so rather than passing vacuously.

use rjt_bench::corpus::File;

#[test]
fn the_intended_kernel_is_reached_on_this_architecture() {
    if !turbo::counters::enabled() {
        println!("skipped: build without count-work carries no counters");
        return;
    }

    // ---- 1. WHICH KERNEL, and does it match the architecture? -------------
    let isa = turbo::counters::isa();
    let available = turbo::counters::isa_available();
    println!(
        "arch={} pointer_width={} isa={isa} isa_available={available} accel={}",
        std::env::consts::ARCH,
        usize::BITS,
        turbo::counters::accel_linked(),
    );

    // The island is OPT-IN (it lost to upstream on 13 of 15 cells at M3), so
    // without it the answer must be the in-crate SWAR on EVERY architecture.
    // `RJT_ISA` can narrow it further, which is why the override is honoured
    // here rather than asserted away.
    let overridden = std::env::var("RJT_ISA").is_ok();
    if !overridden {
        let expected: &[&str] = if cfg!(target_arch = "x86_64") {
            // With the island linked, x86_64 reaches SSE2 or AVX2; without it,
            // SWAR. Both are legitimate; what is NOT legitimate is "scalar".
            &["swar", "sse2", "avx2"]
        } else {
            // Nothing but SWAR exists off x86_64 -- there is no NEON or
            // simd128 kernel, by measurement rather than omission (LEDGER
            // 2026-09-10, M6). The census is what makes that a checked claim:
            // if a future NEON twin lands and is not reached, this fails.
            &["swar"]
        };
        assert!(
            expected.contains(&isa),
            "{} reports isa={isa}, expected one of {expected:?} -- a 'scalar' \
             reading here means every wide scanner degraded silently",
            std::env::consts::ARCH
        );
    }

    // ---- 2. THE WHITESPACE SCANNER ran, on a document that has whitespace -
    // `citm_catalog` is indented, so a scanner that never fires would show
    // zero bytes skipped while the document still parsed correctly.
    let citm = File::CitmCatalog.load();
    turbo::counters::reset();
    let v: turbo::Value = turbo::from_slice(&citm).expect("citm parses");
    let c = turbo::counters::snapshot();
    std::hint::black_box(&v);
    assert!(c.ws_runs > 0, "the whitespace scanner never ran");
    assert!(
        c.ws_bytes > 100_000,
        "only {} whitespace bytes skipped in {} of indented JSON -- the \
         scanner is not seeing the document",
        c.ws_bytes,
        citm.len()
    );
    println!(
        "whitespace: {} runs, {} bytes ({:.1} bytes per run)",
        c.ws_runs,
        c.ws_bytes,
        c.ws_bytes as f64 / c.ws_runs as f64
    );

    // ---- 3. THE EIGHT-DIGIT FOLD ran and HIT ------------------------------
    // `canada` is 2.25 MB of coordinates: if brick B4's chunk path is not
    // reached on this architecture, every number still parses -- one digit at
    // a time.
    let canada = File::Canada.load();
    turbo::counters::reset();
    let v: turbo::Value = turbo::from_slice(&canada).expect("canada parses");
    let c = turbo::counters::snapshot();
    std::hint::black_box(&v);
    assert!(c.d8_calls > 0, "the eight-digit fold was never called");
    assert!(
        c.d8_hits > 0,
        "the eight-digit fold was called {} times and HIT NEVER -- on this \
         architecture it is pure overhead",
        c.d8_calls
    );
    println!(
        "eight-digit fold: {}/{} calls hit ({:.1}%)",
        c.d8_hits,
        c.d8_calls,
        c.d8_hits as f64 / c.d8_calls as f64 * 100.0
    );

    // ---- 4. THE ESCAPE SCANNER is WIDE, not degraded ---------------------
    // This is the sharp one, and getting it right took two attempts. The
    // obvious assertion -- "the wide path covers 8 bytes per step" -- is
    // WRONG, and this census failed on it: `twitter` measures 3.63 bytes per
    // step with the wide path fully engaged. The reason is that
    // `scan_to_escape` has no scalar peel (deliberately: see `ser.rs`), so a
    // string of length L costs `L / 8` wide steps plus `L % 8` TAIL steps --
    // and twitter's mean string is 20 bytes, which is two chunks and a
    // four-byte tail. Six steps for twenty bytes is 3.3, not 8.
    //
    // So the census predicts the step count EXACTLY from the string lengths
    // in the document, and compares. That distinguishes the two paths without
    // a magic ratio:
    //
    //   wide path:   sum over strings of (L / 8 + L % 8)
    //   scalar path: sum over strings of L          <- one step per byte
    //
    // On twitter those are ~101k and ~368k, so there is no way to confuse
    // them, and a future peel or a wider kernel changes the prediction rather
    // than silently passing.
    let twitter = File::Twitter.load();
    let v: turbo::Value = turbo::from_slice(&twitter).expect("twitter parses");

    let mut want_bytes = 0u64;
    let mut want_steps = 0u64;
    fn note(s: &str, bytes: &mut u64, steps: &mut u64) {
        let l = s.len() as u64;
        *bytes += l;
        // The wide path: one step per full 8-byte chunk, then one per tail
        // byte, because `scan_to_escape` has no peel.
        *steps += l / 8 + l % 8;
    }
    fn predict(v: &turbo::Value, bytes: &mut u64, steps: &mut u64) {
        match v {
            turbo::Value::String(s) => note(s, bytes, steps),
            turbo::Value::Object(m) => {
                for (k, val) in m {
                    note(k, bytes, steps);
                    predict(val, bytes, steps);
                }
            }
            turbo::Value::Array(a) => {
                for val in a {
                    predict(val, bytes, steps);
                }
            }
            _ => {}
        }
    }
    predict(&v, &mut want_bytes, &mut want_steps);

    turbo::counters::reset();
    let out = turbo::to_string(&v).expect("twitter stringifies");
    let c = turbo::counters::snapshot();
    std::hint::black_box(&out);

    // Every string byte the serializer wrote is accounted for. If this is
    // wrong, the counter is not seeing the whole document and nothing below
    // means anything.
    assert_eq!(
        c.esc_bytes, want_bytes,
        "the escape scanner saw {} string bytes; the document holds {}",
        c.esc_bytes, want_bytes
    );

    // ONE COUNTER, TWO MEANINGS -- and this census is what found that out.
    //
    // `ESC_STEPS` is incremented by THIS CRATE's scanner once per 8-byte
    // chunk, and by the ISLAND once per call: the island is a separate
    // `no_std` crate with no access to the parent's statics, so it cannot
    // count chunks even in principle. The same `twitter` document reads
    // 101,382 steps without `accel` and 19,327 with it. So the prediction
    // above applies only to the in-crate path, and the census has to know
    // which build it is looking at -- `counters::accel_linked()` exists for
    // this.
    let scalar_would_be = want_bytes;
    if turbo::counters::accel_linked() {
        // With the island, the only reachability claim this counter can
        // support is "the scanner was entered about once per string, not once
        // per byte". `ESC_FRAGS` counts the clean runs handed to the writer,
        // which is the right order of magnitude to compare against.
        assert!(
            c.esc_steps > 0,
            "the island's escape scanner was never entered"
        );
        assert!(
            c.esc_steps * 4 <= want_steps,
            "with the island linked the escape scanner should be entered ~once \
             per string ({} fragments), not per chunk; got {} steps against an \
             in-crate prediction of {}",
            c.esc_frags,
            c.esc_steps,
            want_steps
        );
    } else {
        // Escapes restart the scan, so the prediction is a lower bound --
        // twitter needs an escape on 0.33% of its string bytes. 10% of
        // headroom is far more than that and still nowhere near the scalar
        // count.
        assert!(
            c.esc_steps >= want_steps && c.esc_steps <= want_steps + want_steps / 10,
            "escape scanner took {} steps; the wide path predicts {} (+escape \
             restarts) and the per-byte fallback would take {}",
            c.esc_steps,
            want_steps,
            scalar_would_be
        );
    }
    println!(
        "escape scan: {} steps for {} bytes = {:.2} bytes/step; in-crate wide \
         path predicts {}, per-byte fallback would be {} ({} bytes needed \
         escaping, {} fragments)",
        c.esc_steps,
        c.esc_bytes,
        c.esc_bytes as f64 / c.esc_steps as f64,
        want_steps,
        scalar_would_be,
        c.esc_hits,
        c.esc_frags
    );

    // ---- 5. AND THE POINTER WIDTH IS WHAT WE THINK ----------------------
    // wasm32 is a 32-bit target, and every buffer index, the recursion guard
    // and `IoRead`'s window arithmetic are `usize`. Everything above ran on
    // this width; recording it is what makes the run's provenance legible.
    println!(
        "census passed on {} with {}-bit usize",
        std::env::consts::ARCH,
        usize::BITS
    );
}
