// THE differential target: ours against upstream serde_json on arbitrary bytes.
//
// Every other fuzz target here asks "does this crash?". This one asks the
// question that actually matters for a fork: "does this still agree with the
// crate it replaces?" -- on the outcome, on the printed bytes, on the error
// text with its line and column, and on the exact bits of every float. A
// divergence is the one bug class a fork can introduce that no
// single-implementation fuzzer, and no test written against our own output,
// can see.
#![no_main]

use libfuzzer_sys::fuzz_target;

fn ours(data: &[u8]) -> String {
    match serde_json::from_slice::<serde_json::Value>(data) {
        Ok(v) => match serde_json::to_string(&v) {
            Ok(s) => format!("Ok({s})"),
            Err(e) => format!("SerErr({e})"),
        },
        Err(e) => format!("Err({e} @{}:{} {:?})", e.line(), e.column(), e.classify()),
    }
}

fn upstream(data: &[u8]) -> String {
    match serde_json_upstream::from_slice::<serde_json_upstream::Value>(data) {
        Ok(v) => match serde_json_upstream::to_string(&v) {
            Ok(s) => format!("Ok({s})"),
            Err(e) => format!("SerErr({e})"),
        },
        Err(e) => format!("Err({e} @{}:{} {:?})", e.line(), e.column(), e.classify()),
    }
}

fn ours_f64(text: &str) -> String {
    match serde_json::from_str::<f64>(text) {
        Ok(x) => format!("{:016x}", x.to_bits()),
        Err(e) => format!("Err({e})"),
    }
}

fn upstream_f64(text: &str) -> String {
    match serde_json_upstream::from_str::<f64>(text) {
        Ok(x) => format!("{:016x}", x.to_bits()),
        Err(e) => format!("Err({e})"),
    }
}

fuzz_target!(|data: &[u8]| {
    assert_eq!(ours(data), upstream(data), "divergence from upstream");
    // Numbers get their own comparison: a float that parses to a neighbouring
    // bit pattern prints identically at low precision, so comparing the
    // Value's text alone would miss it.
    if let Ok(text) = std::str::from_utf8(data) {
        assert_eq!(
            ours_f64(text),
            upstream_f64(text),
            "float bit divergence from upstream"
        );
    }
});
