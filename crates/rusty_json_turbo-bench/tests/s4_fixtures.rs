//! The S4 fixture gate: every `corpus/s4/*.json` document must survive a
//! struct round-trip with nothing lost.
//!
//! For each file: read the bytes, `from_slice` into the fixture type, `to_vec`
//! the result, then parse BOTH byte strings as `turbo::Value` and compare. The
//! two byte strings are deliberately NOT compared -- key order and whitespace
//! legitimately differ through a struct round-trip. `Value` equality is the
//! property that matters, because a field the fixture failed to model
//! disappears from the re-serialised bytes and surfaces here as a dropped key.
//!
//! A `flatten` tail and `deny_unknown_fields` cannot coexist, so the fixtures
//! carry no `deny_unknown_fields`; this test is what stands in for it.

use std::collections::BTreeSet;

use rjt_bench::corpus;
use rjt_bench::{
    s4_frame_telemetry, s4_media_probe, s4_node_config, s4_signin_batch, s4_sync_envelope,
    s4_vault_shard,
};
// Only used by the case that `arbitrary_precision` skips (see below).
#[cfg(not(feature = "arbitrary_precision"))]
use rjt_bench::s4_ocr_i18n;
use serde::de::DeserializeOwned;
use serde::Serialize;
use turbo::Value;

fn load(name: &str) -> Vec<u8> {
    let path = corpus::corpus_dir().join("s4").join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// The first place the round-tripped document differs from the original, as a
/// JSON path. `None` means the two documents are equal.
///
/// Objects are compared by key set and then key by key, which normalises key
/// order whether or not `preserve_order` is on. Numbers are compared as
/// `turbo::Number`, so an integer field typed `f64` (or the reverse) is a
/// mismatch and not a rounding curiosity.
fn first_difference(orig: &Value, back: &Value, path: &str) -> Option<String> {
    match (orig, back) {
        (Value::Null, Value::Null) => None,
        (Value::Bool(a), Value::Bool(b)) => {
            if a == b {
                None
            } else {
                Some(format!("{path}: bool {a} became {b}"))
            }
        }
        (Value::Number(a), Value::Number(b)) => {
            if numbers_agree(a, b) {
                None
            } else {
                Some(format!("{path}: number {a} became {b}"))
            }
        }
        (Value::String(a), Value::String(b)) => {
            if a == b {
                None
            } else {
                Some(format!(
                    "{path}: string of {} bytes became {} bytes",
                    a.len(),
                    b.len()
                ))
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return Some(format!("{path}: array of {} became {}", a.len(), b.len()));
            }
            a.iter()
                .zip(b.iter())
                .enumerate()
                .find_map(|(i, (x, y))| first_difference(x, y, &format!("{path}[{i}]")))
        }
        (Value::Object(a), Value::Object(b)) => {
            let ak: BTreeSet<&str> = a.keys().map(String::as_str).collect();
            let bk: BTreeSet<&str> = b.keys().map(String::as_str).collect();
            if let Some(k) = ak.difference(&bk).next() {
                return Some(format!("{path}: key {k:?} was DROPPED by the fixture"));
            }
            if let Some(k) = bk.difference(&ak).next() {
                return Some(format!("{path}: key {k:?} was INVENTED by the fixture"));
            }
            ak.into_iter().find_map(|k| {
                first_difference(
                    a.get(k).expect("key present"),
                    b.get(k).expect("key present"),
                    &format!("{path}.{k}"),
                )
            })
        }
        _ => Some(format!("{path}: {} became {}", kind(orig), kind(back))),
    }
}

/// Read `name`, deserialise it as `T`, re-serialise, and assert the two
/// documents are the same JSON. Returns (input bytes, output bytes).
/// Do two `Number`s represent the same value?
///
/// Normally this is `==`, which compares an integer to a float as unequal --
/// exactly what this gate wants, since a field typed `f64` that holds an
/// integer on the wire is a fixture bug and not a rounding curiosity.
///
/// Under `arbitrary_precision` a `Number` keeps the ORIGINAL DIGITS, so `==`
/// becomes string equality and starts reporting differences that are not
/// differences: `corpus/s4/s4-frame-telemetry.json` carries `0.95920`, which
/// any `f64` field re-serialises as `0.9592`. The value is identical and the
/// trailing zero is formatting. So under that feature the comparison is made
/// on the parsed value, while the integer-versus-float distinction is kept by
/// comparing `is_f64` first.
///
/// This is the same trap the differential oracle hit on day one, from the
/// other side: the float path deliberately does not round-trip its text, so a
/// test that expects printed digits to survive is testing the wrong property.
fn numbers_agree(a: &turbo::Number, b: &turbo::Number) -> bool {
    if a == b {
        return true;
    }
    #[cfg(feature = "arbitrary_precision")]
    {
        // Still a mismatch if one is integral and the other is not: that is
        // the fixture-typing error this gate exists to catch.
        if a.is_f64() != b.is_f64() || a.is_i64() != b.is_i64() || a.is_u64() != b.is_u64() {
            return false;
        }
        if let (Some(x), Some(y)) = (a.as_f64(), b.as_f64()) {
            return x == y;
        }
    }
    false
}

fn round_trip<T: DeserializeOwned + Serialize>(name: &str) -> (usize, usize) {
    let bytes = load(name);
    let value: T = turbo::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("{name}: from_slice into the fixture type failed: {e}"));
    let back = turbo::to_vec(&value).unwrap_or_else(|e| panic!("{name}: to_vec failed: {e}"));

    let orig: Value =
        turbo::from_slice(&bytes).unwrap_or_else(|e| panic!("{name}: original is not JSON: {e}"));
    let rt: Value = turbo::from_slice(&back)
        .unwrap_or_else(|e| panic!("{name}: re-serialised output is not JSON: {e}"));

    if let Some(diff) = first_difference(&orig, &rt, "$") {
        panic!(
            "{name}: the struct round-trip is LOSSY.\n  {diff}\n  original {} bytes, \
             re-serialised {} bytes",
            bytes.len(),
            back.len()
        );
    }
    (bytes.len(), back.len())
}

macro_rules! s4_case {
    ($test:ident, $file:expr, $ty:ty) => {
        #[test]
        fn $test() {
            let (input, output) = round_trip::<$ty>($file);
            println!("{}: {} in, {} out, Value-equal", $file, input, output);
        }
    };
}

s4_case!(
    media_probe_round_trips,
    "s4-media-probe.json",
    s4_media_probe::MediaProbe
);
s4_case!(
    frame_telemetry_round_trips,
    "s4-frame-telemetry.json",
    s4_frame_telemetry::FrameTelemetry
);
s4_case!(
    signin_batch_round_trips,
    "s4-signin-batch.json",
    s4_signin_batch::SigninBatch
);
s4_case!(
    sync_envelope_round_trips,
    "s4-sync-envelope.json",
    s4_sync_envelope::SyncEnvelope
);
s4_case!(
    node_config_round_trips,
    "s4-node-config.json",
    s4_node_config::NodeConfig
);
s4_case!(
    vault_shard_round_trips,
    "s4-vault-shard.json",
    s4_vault_shard::VaultShard
);
// `OcrI18n` is an internally-tagged enum with an `f64` behind the tag, and
// that combination cannot be deserialised under `arbitrary_precision` -- by
// UPSTREAM's design, not this fork's. A tagged enum makes serde buffer the
// content before replaying it, and under that feature a buffered number
// becomes a map, so replaying it into an `f64` field fails with "invalid type:
// map, expected f64".
//
// This was checked rather than assumed: `tests/ap_probe.rs` puts the minimal
// case through BOTH crates and asserts they fail identically, with the same
// message, and that the same float field outside a tag still works in both.
// The fork's contract is byte-identical behaviour, and identical failure is
// part of that -- so the right response here is to skip the case and keep the
// probe, not to reshape the fixture until a feature upstream cannot support
// appears to work.
#[cfg(not(feature = "arbitrary_precision"))]
s4_case!(
    ocr_i18n_round_trips,
    "s4-ocr-i18n.json",
    s4_ocr_i18n::OcrI18n
);
