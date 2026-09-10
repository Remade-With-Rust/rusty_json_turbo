//! PRICING B11 -- the value model -- against the whole program.
//!
//! The `scan` column says value construction is 70-85% of `dom-parse`: a 3.29x
//! ceiling on `citm_catalog` and **6.89x on `s4-frame-telemetry`**. This asks
//! which part of the value model that is, by swapping ONE representation at a
//! time and timing the whole parse.
//!
//! Arms, and what each one changes from the control:
//!
//! - `real` -- `serde_json::Value` itself, the thing we ship
//! - `Base` -- the probe's own code in the same shape (owned key per
//!   occurrence, `BTreeMap`, heap string). Its gap to `real` is the probe's own
//!   overhead, and it is what the other arms are measured against so that
//!   overhead cancels.
//! - `Intern` -- interned keys, nothing else changed
//! - `VecMap` -- a sorted `Vec` instead of a `BTreeMap`, nothing else changed
//! - `All` -- all three levers, including inline short strings
//! - `scan` -- builds nothing; the floor nobody can beat
//!
//! Reported, not asserted, except for work parity: every arm must see the same
//! number of keys, or they did not do the same job.
//!
//! Run it in release or the numbers are about the debug build:
//!
//! ```text
//! cargo test --release -p rusty_json_turbo-bench --test b11_price -- --nocapture
//! ```

use std::time::Instant;

use rjt_bench::b11_probe::{self as probe, All, Base, Intern, Model, VecMap};
use rjt_bench::corpus::File;

/// Best of `n`, because noise can only inflate a timing sample.
fn best_of<F: FnMut() -> u64>(mut f: F, n: usize) -> u64 {
    let mut best = u64::MAX;
    for _ in 0..n {
        best = best.min(f());
    }
    best
}

fn time_model<M: Model>(bytes: &[u8], reps: usize) -> (u64, usize) {
    let mut keys = 0;
    let ns = best_of(
        || {
            let t = Instant::now();
            let (v, k) = probe::parse::<M>(bytes);
            let ns = t.elapsed().as_nanos() as u64;
            keys = k.distinct();
            std::hint::black_box(&v);
            std::hint::black_box(&k);
            ns
        },
        reps,
    );
    (ns, keys)
}

/// Count every key OCCURRENCE, so work parity is checkable across arms that
/// store keys completely differently.
fn key_occurrences(v: &turbo::Value) -> usize {
    match v {
        turbo::Value::Object(m) => m.len() + m.values().map(key_occurrences).sum::<usize>(),
        turbo::Value::Array(a) => a.iter().map(key_occurrences).sum(),
        _ => 0,
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: this is a timing probe")]
fn price_the_value_model() {
    const REPS: usize = 9;

    println!(
        "\n{:<22}{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}   keys (occurrences/distinct)",
        "file", "scan", "real", "Base", "Intern", "VecMap", "All",
    );
    println!("{}", "-".repeat(118));

    for file in [
        File::S4FrameTelemetry,
        File::CitmCatalog,
        File::Twitter,
        File::Canada,
    ] {
        let bytes = file.load();

        // The floor: walk and build nothing.
        let scan = best_of(
            || {
                let t = Instant::now();
                let n: serde::de::IgnoredAny =
                    turbo::from_slice(&bytes).expect("scan must succeed");
                let ns = t.elapsed().as_nanos() as u64;
                std::hint::black_box(n);
                ns
            },
            REPS,
        );

        // What we ship.
        let mut occ = 0;
        let real = best_of(
            || {
                let t = Instant::now();
                let v: turbo::Value = turbo::from_slice(&bytes).expect("real Value");
                let ns = t.elapsed().as_nanos() as u64;
                occ = key_occurrences(&v);
                std::hint::black_box(&v);
                ns
            },
            REPS,
        );

        let (base, _) = time_model::<Base>(&bytes, REPS);
        let (intern, distinct) = time_model::<Intern>(&bytes, REPS);
        let (vecmap, _) = time_model::<VecMap>(&bytes, REPS);
        let (all, distinct_all) = time_model::<All>(&bytes, REPS);

        // WORK PARITY: the two interning arms must agree on how many distinct
        // names the document has, or one of them is not seeing the whole thing.
        assert_eq!(
            distinct,
            distinct_all,
            "{}: interning arms disagree on distinct key count",
            file.name()
        );
        assert!(
            occ >= distinct,
            "{}: fewer occurrences ({occ}) than distinct keys ({distinct})",
            file.name()
        );

        println!(
            "{:<22}{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}   {} / {}  ({:.1}x reuse)",
            file.name(),
            scan,
            real,
            base,
            intern,
            vecmap,
            all,
            occ,
            distinct,
            occ as f64 / distinct.max(1) as f64
        );
        println!(
            "{:<22}{:>10}{:>10.3}{:>10.3}{:>10.3}{:>10.3}{:>10.3}   <- vs Base (>1 = faster than Base)",
            "",
            "",
            base as f64 / real as f64,
            1.0,
            base as f64 / intern as f64,
            base as f64 / vecmap as f64,
            base as f64 / all as f64,
        );
        println!(
            "{:<22}{:>10}{:>10.2}{:>10.2}{:>10.2}{:>10.2}{:>10.2}   <- x above the scan floor",
            "",
            "",
            real as f64 / scan as f64,
            base as f64 / scan as f64,
            intern as f64 / scan as f64,
            vecmap as f64 / scan as f64,
            all as f64 / scan as f64,
        );
    }

    println!(
        "\nmethod: in-process, one binary, best-of-{REPS} per arm; work parity asserted (the two \
         interning arms must agree on the distinct-key count). `Base` is the probe's own code in \
         `Value`'s shape, so the Intern/VecMap/All ratios are measured against IT and the probe's \
         own overhead cancels. A ratio above 1.00 vs Base means the lever helped."
    );
}
