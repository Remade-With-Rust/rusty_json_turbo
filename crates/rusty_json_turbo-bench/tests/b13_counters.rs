//! COUNTING BRICK B13, in its own binary because the counters are global.
//!
//! B13 grows the reader window for a capture longer than it and hands the
//! growth back when the capture ends, and **neither event changes the
//! output**. A gate on bytes cannot tell a window that was returned from one
//! that was kept, so the counters are the only instrument that can -- which
//! makes them load-bearing here rather than confirmatory.
//!
//! The counters are process-global statics, so this cannot share a binary with
//! anything else that parses: `tests/b13_raw_range.rs` runs its six cases in
//! parallel threads and would inflate every reading. One file is one binary,
//! and that is the isolation.
//!
//! Needs `--features raw_value,count-work`; without the latter the library
//! carries no counters and the test says so rather than passing vacuously.
#![cfg(feature = "raw_value")]

use turbo::value::RawValue;

/// The three things only a counter can see: one copy per capture, every byte
/// accounted for, and a growth that is handed back.
#[test]
fn the_counters_say_the_capture_is_one_copy_and_the_growth_is_temporary() {
    if !turbo::counters::enabled() {
        println!("skipped: build without count-work carries no counters");
        return;
    }

    // Ordinary documents: 10,000 captures, every byte of every document
    // accounted for, and NOT ONE growth -- the whole stream fits the window
    // it was given.
    let bytes = rjt_bench::corpus::File::S5LogStream.load();
    let docs = rjt_bench::corpus::stream_documents(rjt_bench::corpus::File::S5LogStream);
    let want_bytes: usize = docs.iter().map(Vec::len).sum();

    turbo::counters::reset();
    let n = turbo::Deserializer::from_reader(&bytes[..])
        .into_iter::<Box<RawValue>>()
        .map(|r| r.expect("streamed").get().len())
        .sum::<usize>();
    let c = turbo::counters::snapshot();
    assert_eq!(n, want_bytes, "captured a different number of bytes");
    assert_eq!(c.raw_captures, 10_000, "one capture per document");
    assert_eq!(
        c.raw_bytes, want_bytes as u64,
        "the counter disagrees with the text"
    );
    assert_eq!(
        c.raw_grows, 0,
        "an ordinary log line must not grow the window"
    );
    assert_eq!(c.raw_shrinks, 0, "nothing grew, so nothing should shrink");
    println!(
        "s5-log-stream: {} captures, {} bytes of raw text, {} window growths; upstream reached that total in {} pushes",
        c.raw_captures, c.raw_bytes, c.raw_grows, c.raw_bytes
    );

    // One value larger than the window: it MUST grow, and every growth must be
    // handed back, or a stream with one big value in it would keep the memory
    // for the rest of its life.
    let big = format!("[\"{}\"]", "b".repeat(60_000));
    turbo::counters::reset();
    let raw: Box<RawValue> = turbo::from_reader(big.as_bytes()).expect("oversized");
    let c = turbo::counters::snapshot();
    assert_eq!(raw.get(), big, "the oversized capture lost bytes");
    assert_eq!(c.raw_captures, 1);
    assert_eq!(c.raw_bytes, big.len() as u64);
    // 16 KiB doubling to hold ~60 KiB: 16 -> 32 -> 64, so two growths.
    assert_eq!(c.raw_grows, 2, "doubling from 16 KiB to hold 60 KiB");
    assert_eq!(c.raw_shrinks, 1, "the growth was not handed back");
    println!(
        "one {}-byte value: {} growths, {} handed back",
        c.raw_bytes, c.raw_grows, c.raw_shrinks
    );
}
