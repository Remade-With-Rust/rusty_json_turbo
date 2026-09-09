// `RawValue` borrows a span of the input and hands it back verbatim. The
// invariant: the span it returns is itself a single well-formed JSON document,
// and re-parsing it yields the same value.
#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_json::{from_slice, from_str, value::RawValue, Value};

fuzz_target!(|data: &[u8]| {
    let Ok(raw) = from_slice::<&RawValue>(data) else {
        return;
    };
    let text = raw.get();
    let from_raw = from_str::<Value>(text).expect("a RawValue's span must be a valid document");
    let direct = from_slice::<Value>(data).expect("if the RawValue parsed, the Value must too");
    assert_eq!(from_raw, direct, "RawValue span must denote the same value");
});
