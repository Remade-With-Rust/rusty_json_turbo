//! PRICING BRICK B13 before building it.
//!
//! B13 is "`RawValue` on `IoRead` as a range copy", and the plan says it
//! "falls out of B7" -- now that the reader holds a window, the raw bytes of a
//! value are a slice of that window rather than something to accumulate one
//! byte at a time.
//!
//! Today `IoRead` pushes every consumed byte into the raw buffer from `next`,
//! `discard` and `skip_whitespace`. On the S5 stream that is 2.6 MB of
//! individual pushes across 10,000 documents. Whether replacing them with one
//! copy per refill is worth anything is a measurement, and this is it: how
//! much does asking for a `RawValue` cost over not asking, on a reader?
//!
//! Reported, not asserted. A test is the cheapest place to put a measurement
//! that needs a corpus file and two crates.
#![cfg(feature = "raw_value")]

use std::time::Instant;

fn best_of<F: FnMut() -> u64>(mut f: F, n: usize) -> u64 {
    let mut best = u64::MAX;
    for _ in 0..n {
        best = best.min(f());
    }
    best
}

#[test]
fn report_the_cost_of_raw_value_over_a_reader() {
    let bytes = rjt_bench::corpus::File::S5LogStream.load();
    let docs = rjt_bench::corpus::stream_documents(rjt_bench::corpus::File::S5LogStream);
    assert_eq!(docs.len(), 10_000);

    let time = |mut f: Box<dyn FnMut()>| -> u64 {
        best_of(
            || {
                let t = Instant::now();
                f();
                t.elapsed().as_nanos() as u64
            },
            7,
        )
    };

    // Value, over a reader: the baseline for "read the whole stream".
    let b = bytes.clone();
    let value_reader = time(Box::new(move || {
        let n = turbo::Deserializer::from_reader(&b[..])
            .into_iter::<turbo::Value>()
            .map(Result::unwrap)
            .count();
        std::hint::black_box(n);
    }));

    // RawValue, over a reader: the same walk, but every consumed byte is also
    // pushed into the raw buffer. The gap is what B13 could attack.
    let b = bytes.clone();
    let raw_reader = time(Box::new(move || {
        let n = turbo::Deserializer::from_reader(&b[..])
            .into_iter::<Box<turbo::value::RawValue>>()
            .map(Result::unwrap)
            .map(|r| r.get().len())
            .sum::<usize>();
        std::hint::black_box(n);
    }));

    // And over a slice, where `RawValue` is already a borrow rather than a
    // copy -- the floor B13 is aiming at.
    let b = bytes.clone();
    let raw_slice = time(Box::new(move || {
        let n = turbo::Deserializer::from_slice(&b[..])
            .into_iter::<Box<turbo::value::RawValue>>()
            .map(Result::unwrap)
            .map(|r| r.get().len())
            .sum::<usize>();
        std::hint::black_box(n);
    }));

    // Upstream's reader, for reference: it has the per-byte push too, on top
    // of a per-byte iterator.
    let b = bytes.clone();
    let up_raw_reader = time(Box::new(move || {
        let n = serde_json_upstream::Deserializer::from_reader(&b[..])
            .into_iter::<Box<serde_json_upstream::value::RawValue>>()
            .map(Result::unwrap)
            .map(|r| r.get().len())
            .sum::<usize>();
        std::hint::black_box(n);
    }));

    let mbps = |ns: u64| bytes.len() as f64 / ns as f64 * 1e3;
    println!(
        "B13 pricing on s5-log-stream ({} bytes, 10,000 docs)",
        bytes.len()
    );
    println!(
        "  Value    / reader   {:>10} ns  {:>6.0} MB/s",
        value_reader,
        mbps(value_reader)
    );
    println!(
        "  RawValue / reader   {:>10} ns  {:>6.0} MB/s",
        raw_reader,
        mbps(raw_reader)
    );
    println!(
        "  RawValue / slice    {:>10} ns  {:>6.0} MB/s",
        raw_slice,
        mbps(raw_slice)
    );
    println!(
        "  RawValue / reader, upstream {:>10} ns  {:>6.0} MB/s",
        up_raw_reader,
        mbps(up_raw_reader)
    );
    println!(
        "  => RawValue over a reader costs {:.2}x a Value over a reader, and {:.2}x the same \
         RawValue over a slice. Ours vs upstream on the reader: {:.3}x",
        raw_reader as f64 / value_reader as f64,
        raw_reader as f64 / raw_slice as f64,
        raw_reader as f64 / up_raw_reader as f64
    );
}
