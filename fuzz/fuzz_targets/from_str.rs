// The `StrRead` path: input already known to be UTF-8, so string parsing takes
// the `from_utf8_unchecked` branch that `from_slice` does not.
#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_json::{from_str, Value};

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        _ = from_str::<Value>(text);
    }
});
