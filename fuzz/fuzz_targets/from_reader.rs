// The `IoRead` path: byte-at-a-time through an `io::Read`, with its own
// line/column tracking and its own scratch handling.
#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_json::{from_reader, Value};

fuzz_target!(|data: &[u8]| {
    _ = from_reader::<_, Value>(std::io::Cursor::new(data));
});
