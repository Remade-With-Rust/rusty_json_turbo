//! GATING BRICK B13: a `RawValue` over a reader is now a RANGE of the window.
//!
//! Before B13, `IoRead` captured a `RawValue` by pushing every consumed byte
//! into a `Vec` from `next`, `discard` and the whitespace override. Now it
//! records where the value starts, holds the window from there across refills,
//! and takes the text in one copy at the end.
//!
//! That moves the risk somewhere new. The old capture could not care where the
//! buffer boundaries fell, because it copied byte by byte; the new one is only
//! correct if **the window really does hold every byte of the value**, however
//! the reader chooses to hand them over. So the cases here are all about
//! boundaries:
//!
//! - a value that spans a refill,
//! - a value LONGER THAN THE WHOLE BUFFER, which has to grow it,
//! - a capture that starts part-way into the window (`raw_from > 0`),
//! - whitespace inside a captured value, which the wide scanner skips
//!   wholesale and no longer records,
//! - a reader that yields ONE BYTE PER `read`, which puts a refill between
//!   almost every byte of the capture.
//!
//! Every case is checked three ways: against upstream serde_json on the same
//! route, against our own `SliceRead` (whose capture was already a range and
//! which B13 did not touch), and against the source text itself.
#![cfg(feature = "raw_value")]

use std::collections::BTreeMap;
use turbo::value::RawValue;

/// A reader that yields at most one byte per `read`, so the parser has to
/// refill constantly and every capture gets split.
struct Dribble<'a>(&'a [u8]);

impl std::io::Read for Dribble<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.0.is_empty() || out.is_empty() {
            return Ok(0);
        }
        out[0] = self.0[0];
        self.0 = &self.0[1..];
        Ok(1)
    }
}

/// A reader that hands over a fixed-size block, chosen to land the boundary
/// somewhere other than where the 8 KiB refill would.
struct Chunked<'a>(&'a [u8], usize);

impl std::io::Read for Chunked<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let n = self.0.len().min(out.len()).min(self.1);
        out[..n].copy_from_slice(&self.0[..n]);
        self.0 = &self.0[n..];
        Ok(n)
    }
}

/// The raw text of one whole document, by every route, all four agreeing.
#[track_caller]
fn raw_agrees(name: &str, doc: &str) {
    let slice: Box<RawValue> = turbo::from_str(doc).expect("ours, slice");
    let reader: Box<RawValue> = turbo::from_reader(doc.as_bytes()).expect("ours, reader");
    let dribble: Box<RawValue> =
        turbo::from_reader(Dribble(doc.as_bytes())).expect("ours, one byte at a time");
    // 8 KiB is `IO_CHUNK`, so 8 KiB - 1 guarantees the block boundary and the
    // refill boundary do not coincide.
    let chunked: Box<RawValue> =
        turbo::from_reader(Chunked(doc.as_bytes(), 8191)).expect("ours, odd blocks");
    let up: Box<serde_json_upstream::value::RawValue> =
        serde_json_upstream::from_reader(doc.as_bytes()).expect("upstream, reader");

    assert_eq!(reader.get(), slice.get(), "{name}: reader != slice");
    assert_eq!(dribble.get(), slice.get(), "{name}: dribbled != slice");
    assert_eq!(chunked.get(), slice.get(), "{name}: odd blocks != slice");
    assert_eq!(reader.get(), up.get(), "{name}: reader != upstream");
    // A whole-document capture is the document, less only the leading and
    // trailing whitespace that is outside the value.
    assert_eq!(slice.get(), doc.trim(), "{name}: not the source text");
}

#[test]
fn a_value_spanning_refills_and_one_larger_than_the_buffer() {
    // 8 KiB is one refill and the buffer is 16 KiB, so these straddle: under,
    // across one boundary, and past the whole buffer twice over, which is the
    // only way to reach B13's growth path.
    for n in [1usize, 100, 1_000, 4_000, 12_000, 40_000] {
        let mut doc = String::from("[");
        for i in 0..n {
            if i > 0 {
                doc.push(',');
            }
            // Five digits and no leading zero (which JSON forbids), so
            // element boundaries are not aligned to any power of two and land
            // inside the refill boundary.
            doc.push_str(&format!("{}", 10_000 + i % 90_000));
        }
        doc.push(']');
        assert!(doc.len() > n * 5, "the fixture must actually be that long");
        raw_agrees(&format!("array of {n}"), &doc);
    }
}

#[test]
fn whitespace_inside_a_captured_value_is_kept_across_a_refill() {
    // The whitespace override skips a run WHOLESALE and no longer records it,
    // so a run long enough to cross a refill is the case that would lose
    // bytes if the window were not held. This is the same failure upstream's
    // `test_boxed_raw_value` caught for the one-byte case.
    for gap in [1usize, 7, 8_000, 9_000, 20_000] {
        let pad = " ".repeat(gap);
        let doc = format!("{{{pad}\"a\":{pad}1,{pad}\"b\":{pad}[2,{pad}3]{pad}}}");
        raw_agrees(&format!("gap of {gap}"), &doc);
    }
}

#[test]
fn a_capture_that_starts_part_way_into_the_window() {
    // `raw_from > 0`: the bytes before the capture are consumed and must NOT
    // appear in it, while the bytes of the value must survive the refills that
    // happen during it. A map of `RawValue` gets both in one document, with a
    // capture starting at a different offset for every key.
    let mut doc = String::from("{");
    let mut expected: Vec<(String, String)> = Vec::new();
    for i in 0..40 {
        if i > 0 {
            doc.push(',');
        }
        let key = format!("k{i:03}");
        // Values of wildly different sizes, so some captures fit inside one
        // window and others span several refills.
        let val = match i % 4 {
            0 => format!("{i}"),
            1 => format!("\"{}\"", "s".repeat(i * 137)),
            2 => format!(
                "[{}]",
                (0..i * 11)
                    .map(|j| j.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            _ => format!("{{\"n\": {i}, \"pad\": \"{}\"}}", "p".repeat(i * 211)),
        };
        doc.push_str(&format!("\"{key}\": {val}"));
        expected.push((key, val));
    }
    doc.push('}');
    assert!(
        doc.len() > 32 * 1024,
        "must be bigger than the whole buffer"
    );

    let by_route: [(&str, BTreeMap<String, Box<RawValue>>); 4] = [
        ("slice", turbo::from_str(&doc).expect("slice")),
        (
            "reader",
            turbo::from_reader(doc.as_bytes()).expect("reader"),
        ),
        (
            "dribble",
            turbo::from_reader(Dribble(doc.as_bytes())).expect("dribble"),
        ),
        (
            "odd blocks",
            turbo::from_reader(Chunked(doc.as_bytes(), 8191)).expect("odd blocks"),
        ),
    ];
    let up: BTreeMap<String, Box<serde_json_upstream::value::RawValue>> =
        serde_json_upstream::from_reader(doc.as_bytes()).expect("upstream");

    for (route, map) in &by_route {
        assert_eq!(map.len(), expected.len(), "{route}: wrong number of keys");
        for (key, val) in &expected {
            let got = map
                .get(key)
                .unwrap_or_else(|| panic!("{route}: {key} missing"));
            assert_eq!(got.get(), val.as_str(), "{route}: {key} captured wrongly");
            assert_eq!(
                got.get(),
                up[key].get(),
                "{route}: {key} differs from upstream"
            );
        }
    }
}

#[test]
fn a_stream_of_raw_values_over_a_reader_matches_the_source_lines() {
    // The S5 corpus member, 10,000 documents: the workload B13 was priced on.
    // Each capture starts wherever the previous one ended, so `raw_from` takes
    // every value it can, and a refill lands inside a document roughly every
    // thirty of them.
    let bytes = rjt_bench::corpus::File::S5LogStream.load();
    let docs = rjt_bench::corpus::stream_documents(rjt_bench::corpus::File::S5LogStream);
    assert_eq!(docs.len(), 10_000);

    let ours: Vec<String> = turbo::Deserializer::from_reader(&bytes[..])
        .into_iter::<Box<RawValue>>()
        .map(|r| r.expect("ours, streamed over a reader").get().to_owned())
        .collect();
    let up: Vec<String> = serde_json_upstream::Deserializer::from_reader(&bytes[..])
        .into_iter::<Box<serde_json_upstream::value::RawValue>>()
        .map(|r| r.expect("upstream, streamed").get().to_owned())
        .collect();

    assert_eq!(ours.len(), docs.len(), "wrong number of documents");
    assert_eq!(ours, up, "our captures differ from upstream's");
    for (i, (got, want)) in ours.iter().zip(&docs).enumerate() {
        assert_eq!(
            got.as_bytes(),
            &want[..],
            "document {i} captured the wrong bytes"
        );
    }
}

#[test]
fn byte_offset_is_unchanged_by_holding_the_window() {
    // B13 stops `fill` from sliding the window while a capture is running,
    // and `byte_offset` is computed from `base + pos`. If holding leaked into
    // that arithmetic, the offsets after each document would drift -- and
    // `StreamDeserializer` uses them to find the next document, so it would
    // drift silently rather than fail.
    let bytes = rjt_bench::corpus::File::S5LogStream.load();

    let offsets = |raw: bool| -> Vec<usize> {
        let mut out = Vec::new();
        if raw {
            let mut it = turbo::Deserializer::from_reader(&bytes[..]).into_iter::<Box<RawValue>>();
            while let Some(item) = it.next() {
                item.expect("raw document");
                out.push(it.byte_offset());
            }
        } else {
            let mut it = turbo::Deserializer::from_reader(&bytes[..]).into_iter::<turbo::Value>();
            while let Some(item) = it.next() {
                item.expect("value document");
                out.push(it.byte_offset());
            }
        }
        out
    };

    let with_raw = offsets(true);
    let with_value = offsets(false);
    assert_eq!(with_raw.len(), 10_000);
    assert_eq!(
        with_raw, with_value,
        "capturing a RawValue moved the stream offsets"
    );

    let mut up = Vec::new();
    let mut it = serde_json_upstream::Deserializer::from_reader(&bytes[..])
        .into_iter::<Box<serde_json_upstream::value::RawValue>>();
    while let Some(item) = it.next() {
        item.expect("upstream raw document");
        up.push(it.byte_offset());
    }
    assert_eq!(with_raw, up, "offsets differ from upstream's");
}

#[test]
fn the_window_recovers_after_capturing_a_value_larger_than_itself() {
    // B13 grows the window for a capture longer than 16 KiB and gives the
    // growth back when the capture ends -- a slide, a truncate and a shrink,
    // performed with unread bytes still in the window. That is real state
    // surgery on a live parser, so it needs both halves checked: what comes
    // AFTER the huge value inside the same document, and what comes after it
    // in the same stream.
    let big = "b".repeat(60_000);
    let doc = format!("{{\"big\": \"{big}\", \"after\": [1, 2, 3], \"tail\": {{\"n\": 7}}}}");
    for route in ["reader", "dribble", "odd blocks"] {
        let map: BTreeMap<String, Box<RawValue>> = match route {
            "reader" => turbo::from_reader(doc.as_bytes()),
            "dribble" => turbo::from_reader(Dribble(doc.as_bytes())),
            _ => turbo::from_reader(Chunked(doc.as_bytes(), 8191)),
        }
        .unwrap_or_else(|e| panic!("{route}: {e}"));
        assert_eq!(map["big"].get().len(), big.len() + 2, "{route}: big value");
        assert_eq!(map["after"].get(), "[1, 2, 3]", "{route}: after the growth");
        assert_eq!(map["tail"].get(), "{\"n\": 7}", "{route}: tail");
    }

    // And a stream: one oversized document, then plenty of ordinary ones, so
    // the recovered window has to keep finding document boundaries.
    let mut stream = format!("[\"{big}\"]\n");
    let mut want = vec![format!("[\"{big}\"]")];
    for i in 0..200 {
        stream.push_str(&format!("{{\"i\": {i}}}\n"));
        want.push(format!("{{\"i\": {i}}}"));
    }
    let got: Vec<String> = turbo::Deserializer::from_reader(stream.as_bytes())
        .into_iter::<Box<RawValue>>()
        .map(|r| r.expect("streamed after a growth").get().to_owned())
        .collect();
    assert_eq!(got, want, "the stream lost its way after the window grew");

    let up: Vec<String> = serde_json_upstream::Deserializer::from_reader(stream.as_bytes())
        .into_iter::<Box<serde_json_upstream::value::RawValue>>()
        .map(|r| r.expect("upstream").get().to_owned())
        .collect();
    assert_eq!(got, up, "differs from upstream after the window grew");
}
