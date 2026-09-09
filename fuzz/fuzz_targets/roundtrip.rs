// A value that parsed must serialize, and what it serializes must parse again.
//
// The second parse is NOT required to equal the first: the default float path
// is deliberately not round-trip exact, so `1e-7` can print and re-read as a
// neighbouring f64. What IS required is that printing is total (never fails on
// a value we produced) and that the text we print is always something we can
// read back -- a parser that emits what it cannot consume is broken even
// though every single-direction test passes.
#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_json::{from_slice, from_str, to_string, Value};

fuzz_target!(|data: &[u8]| {
    let Ok(value) = from_slice::<Value>(data) else {
        return;
    };
    let text = to_string(&value).expect("serializing a parsed Value must not fail");
    let again = from_str::<Value>(&text).expect("our own output must parse");
    // Printing is idempotent from the second generation on: whatever float the
    // re-parse settled on must print the same way again.
    let text2 = to_string(&again).expect("serializing must not fail");
    let third = from_str::<Value>(&text2).expect("our own output must parse");
    assert_eq!(
        to_string(&third).unwrap(),
        text2,
        "printing must reach a fixed point after one re-parse"
    );
});
