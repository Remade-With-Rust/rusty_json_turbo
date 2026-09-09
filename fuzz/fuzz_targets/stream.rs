// `StreamDeserializer` over concatenated documents: the byte offsets it
// reports have to stay inside the input and move forward, or a caller slicing
// on them reads the wrong bytes (or panics).
#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_json::{Deserializer, Value};

fuzz_target!(|data: &[u8]| {
    let mut iter = Deserializer::from_slice(data).into_iter::<Value>();
    let mut last = 0usize;
    loop {
        let item = iter.next();
        let offset = iter.byte_offset();
        assert!(offset <= data.len(), "byte_offset ran past the input");
        assert!(offset >= last, "byte_offset went backwards");
        last = offset;
        match item {
            None => break,
            Some(Ok(_)) => {}
            Some(Err(_)) => break,
        }
    }
});
