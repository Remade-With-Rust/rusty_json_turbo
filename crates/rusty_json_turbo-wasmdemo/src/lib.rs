//! THE BROWSER-TARGET CHECKSUM GATE (M6).
//!
//! `wasm32-wasip1` proves the parser works when there is a platform underneath
//! it. **A browser is not that.** `wasm32-unknown-unknown` has no filesystem,
//! no clock, no environment and no WASI syscalls, and it is the target a page
//! actually loads -- so a claim about running in a browser has to be made on
//! it, not on WASI.
//!
//! This crate is that claim, reduced to something a JS host can check in one
//! call: it embeds real corpus documents with `include_bytes!` (there is no
//! filesystem to read them from), parses each into a [`turbo::Value`],
//! re-serializes it, and returns a checksum of the OUTPUT BYTES. The native
//! test computes the same checksum from the same source, and `tools/wasmdemo.mjs`
//! runs the module under Node and prints it. Equal checksums mean the parser
//! and the serializer produced byte-identical output in a browser-shaped
//! environment.
//!
//! The three documents are chosen so the checksum covers the three value
//! families that could differ across targets rather than one:
//!
//! - `s4-node-config` -- small, and dense in booleans and nulls;
//! - `twitter` -- 367,917 bytes of string content, escapes and non-ASCII, so
//!   the escape scanner and the unescaper both have to agree;
//! - `canada` -- 2.25 MB of floating point, which is the family most likely to
//!   differ between targets, and the only one where a difference would be a
//!   silent wrong answer rather than a crash.
//!
//! The parser is linked **alloc-only** (`default-features = false`), because a
//! browser build has no business pulling `std`'s platform shims into a JSON
//! parser -- and because this is the only gate that proves the alloc-only
//! configuration produces correct BYTES on the browser target rather than
//! merely compiling for it.

extern crate alloc;

use alloc::string::String;

/// The embedded documents, in the order the exported functions index them.
///
/// `include_bytes!` rather than a file read, because on this target there is
/// nothing to read from.
const DOCS: [(&str, &[u8]); 3] = [
    (
        "s4-node-config",
        include_bytes!("../../../corpus/s4/s4-node-config.json"),
    ),
    ("twitter", include_bytes!("../../../corpus/twitter.json")),
    ("canada", include_bytes!("../../../corpus/canada.json")),
];

/// FNV-1a over 64 bits.
///
/// Chosen because it is **exactly reproducible with integer arithmetic
/// alone**: no floating point, no table, no dependence on pointer width or
/// byte order. A checksum whose value could itself vary by target would prove
/// nothing here. It is not cryptographic and does not need to be -- the
/// question is "are these two byte strings the same", asked across a language
/// boundary that cannot pass the byte strings themselves cheaply.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Parse document `which` and re-serialize it, returning the output and its
/// checksum. Shared by the wasm exports and the native test so the two cannot
/// drift.
///
/// # Panics
///
/// If a corpus document does not parse or does not re-serialize. Both are
/// gated elsewhere; here a panic is the correct answer, because a browser
/// getting a parse error on `twitter.json` is not a result to report politely.
#[must_use]
pub fn round_trip(which: usize) -> (String, u64) {
    let (_, bytes) = DOCS[which];
    let value: turbo::Value = turbo::from_slice(bytes).expect("a corpus document must parse");
    let out = turbo::to_string(&value).expect("a Value must re-serialize");
    let sum = fnv1a(out.as_bytes());
    (out, sum)
}

/// Name of document `which`.
#[must_use]
pub fn name(which: usize) -> &'static str {
    DOCS[which].0
}

/// How many documents are embedded.
#[must_use]
pub fn count() -> usize {
    DOCS.len()
}

/// One checksum over every document, so a JS host can compare a single number.
///
/// Folded in order, with each document's own checksum mixed in, so a
/// difference in any one of them changes the total.
#[must_use]
pub fn checksum_all() -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for i in 0..DOCS.len() {
        let (_, sum) = round_trip(i);
        h ^= sum;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ---------------------------------------------------------------------------
// The wasm surface. Four integer-in, integer-out functions: everything a JS
// host needs, with no strings crossing the boundary, so there is no allocator
// protocol for the two sides to agree on and nothing to get wrong.
//
// `#[no_mangle]` is denied by this crate's `unsafe_code` lint and the lint is
// RIGHT: overriding a symbol name can collide with another library's, and the
// linker's behaviour then is undefined. It is allowed here per item rather
// than crate-wide, because these four names are the entire reason the crate
// exists -- a browser can only call an export it can name -- and the `rjt_`
// prefix is what keeps them from colliding. Nothing else in this crate may
// use it without the same justification.
// ---------------------------------------------------------------------------

/// How many documents this module carries.
#[allow(unsafe_code)] // one of the four named exports; see above
#[no_mangle]
pub extern "C" fn rjt_doc_count() -> u32 {
    count() as u32
}

/// Checksum of document `which`'s re-serialized bytes.
#[allow(unsafe_code)] // one of the four named exports; see above
#[no_mangle]
pub extern "C" fn rjt_checksum(which: u32) -> u64 {
    round_trip(which as usize).1
}

/// Length in bytes of document `which`'s re-serialized output.
///
/// Carried alongside the checksum because a length mismatch localises a fault
/// far faster than a hash mismatch does.
#[allow(unsafe_code)] // one of the four named exports; see above
#[no_mangle]
pub extern "C" fn rjt_output_len(which: u32) -> u32 {
    round_trip(which as usize).0.len() as u32
}

/// One checksum over all of them.
#[allow(unsafe_code)] // one of the four named exports; see above
#[no_mangle]
pub extern "C" fn rjt_checksum_all() -> u64 {
    checksum_all()
}
