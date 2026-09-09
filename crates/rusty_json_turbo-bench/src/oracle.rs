//! The differential gate: ours against upstream serde_json, in-process.
//!
//! For every document both crates must agree on:
//! - the outcome of `from_slice`, `from_str` and `from_reader` into `Value`:
//!   `Ok` with identical compact and pretty bytes, or `Err` with identical
//!   message text, line, column and category;
//! - cross-parsing: each crate parses the other's compact output back to the
//!   same bytes;
//! - `StreamDeserializer` over the bytes: the same items and byte offsets;
//! - every number token: identical `f64`/`f32` BIT PATTERNS, identical
//!   `u64`/`i64`/`u128`/`i128` outcomes, identical `Value` text.
//!
//! Everything is compared as strings built the same way on both sides, so a
//! mismatch prints exactly what differed. Nothing here is a fuzzer; it is the
//! gate every brick must pass over the whole corpus, every commit.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::Cursor;

/// One disagreement between the two crates.
#[derive(Debug, Clone)]
pub struct Mismatch {
    pub doc: String,
    pub check: &'static str,
    pub ours: String,
    pub upstream: String,
}

impl std::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {}\n    ours:     {}\n    upstream: {}",
            self.doc,
            self.check,
            truncate(&self.ours),
            truncate(&self.upstream)
        )
    }
}

fn truncate(s: &str) -> String {
    const MAX: usize = 300;
    if s.len() <= MAX {
        s.to_owned()
    } else {
        let mut end = MAX;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}... ({} bytes)", &s[..end], s.len())
    }
}

/// Compare two descriptions; record a mismatch if they differ.
pub fn compare(
    out: &mut Vec<Mismatch>,
    doc: &str,
    check: &'static str,
    ours: String,
    upstream: String,
) {
    if ours != upstream {
        out.push(Mismatch {
            doc: doc.to_owned(),
            check,
            ours,
            upstream,
        });
    }
}

// ---- outcome descriptions, built identically on both sides ----------------

fn err_t(e: &turbo::Error) -> String {
    format!("Err({e} @{}:{} {:?})", e.line(), e.column(), e.classify())
}

fn err_u(e: &serde_json_upstream::Error) -> String {
    format!("Err({e} @{}:{} {:?})", e.line(), e.column(), e.classify())
}

fn value_t(r: &Result<turbo::Value, turbo::Error>) -> String {
    match r {
        Ok(v) => format!("Ok({v})"),
        Err(e) => err_t(e),
    }
}

fn value_u(r: &Result<serde_json_upstream::Value, serde_json_upstream::Error>) -> String {
    match r {
        Ok(v) => format!("Ok({v})"),
        Err(e) => err_u(e),
    }
}

fn pretty_t(r: &Result<turbo::Value, turbo::Error>) -> String {
    match r {
        Ok(v) => turbo::to_string_pretty(v).map_or_else(|e| err_t(&e), |s| format!("Ok({s})")),
        Err(e) => err_t(e),
    }
}

fn pretty_u(r: &Result<serde_json_upstream::Value, serde_json_upstream::Error>) -> String {
    match r {
        Ok(v) => serde_json_upstream::to_string_pretty(v)
            .map_or_else(|e| err_u(&e), |s| format!("Ok({s})")),
        Err(e) => err_u(e),
    }
}

fn stream_t(bytes: &[u8]) -> String {
    let mut s = String::new();
    let mut it = turbo::Deserializer::from_slice(bytes).into_iter::<turbo::Value>();
    loop {
        match it.next() {
            None => break,
            Some(Ok(v)) => {
                let _ = write!(s, "Ok({v})@{};", it.byte_offset());
            }
            Some(Err(e)) => {
                let _ = write!(s, "{}@{};", err_t(&e), it.byte_offset());
                break;
            }
        }
    }
    s
}

fn stream_u(bytes: &[u8]) -> String {
    let mut s = String::new();
    let mut it = serde_json_upstream::Deserializer::from_slice(bytes)
        .into_iter::<serde_json_upstream::Value>();
    loop {
        match it.next() {
            None => break,
            Some(Ok(v)) => {
                let _ = write!(s, "Ok({v})@{};", it.byte_offset());
            }
            Some(Err(e)) => {
                let _ = write!(s, "{}@{};", err_u(&e), it.byte_offset());
                break;
            }
        }
    }
    s
}

// ---- the document gate ----------------------------------------------------

/// Every check for one document. Returns the mismatches (empty = identical).
pub fn check_document(name: &str, bytes: &[u8]) -> Vec<Mismatch> {
    let mut out = Vec::new();

    // 1. from_slice into Value: outcome, compact bytes, pretty bytes.
    let rt = turbo::from_slice::<turbo::Value>(bytes);
    let ru = serde_json_upstream::from_slice::<serde_json_upstream::Value>(bytes);
    compare(
        &mut out,
        name,
        "from_slice::<Value>",
        value_t(&rt),
        value_u(&ru),
    );
    compare(
        &mut out,
        name,
        "to_string_pretty(from_slice)",
        pretty_t(&rt),
        pretty_u(&ru),
    );

    if let (Ok(vt), Ok(vu)) = (&rt, &ru) {
        // to_vec must equal to_string's bytes on each side, and across sides.
        let bt = turbo::to_vec(vt).expect("to_vec");
        let bu = serde_json_upstream::to_vec(vu).expect("to_vec");
        compare(
            &mut out,
            name,
            "to_vec",
            String::from_utf8_lossy(&bt).into_owned(),
            String::from_utf8_lossy(&bu).into_owned(),
        );

        // 2. Re-parse and cross-parse. NOT "the text re-parses to itself":
        // the default float path is deliberately not round-trip exact (that
        // is what `float_roundtrip` is for), so a printed value can re-parse
        // to a neighbouring f64 and print shorter -- canada.json loses 8 KB on
        // one round trip. The contract is that BOTH crates do exactly that,
        // so each side's re-parse is compared to the other side's re-parse.
        let st = vt.to_string();
        let su = vu.to_string();
        let again_t = turbo::from_str::<turbo::Value>(&st);
        let again_u = serde_json_upstream::from_str::<serde_json_upstream::Value>(&su);
        compare(
            &mut out,
            name,
            "re-parse own text",
            value_t(&again_t),
            value_u(&again_u),
        );
        let cross_t = turbo::from_str::<turbo::Value>(&su);
        let cross_u = serde_json_upstream::from_str::<serde_json_upstream::Value>(&st);
        compare(
            &mut out,
            name,
            "cross-parse the other side's text",
            value_t(&cross_t),
            value_u(&cross_u),
        );
    }

    // 3. from_str, when the bytes are UTF-8 (a different Read impl: StrRead).
    if let Ok(text) = std::str::from_utf8(bytes) {
        let rt = turbo::from_str::<turbo::Value>(text);
        let ru = serde_json_upstream::from_str::<serde_json_upstream::Value>(text);
        compare(
            &mut out,
            name,
            "from_str::<Value>",
            value_t(&rt),
            value_u(&ru),
        );
    }

    // 4. from_reader (IoRead: the byte-at-a-time path with its own line/col).
    let rt = turbo::from_reader::<_, turbo::Value>(Cursor::new(bytes));
    let ru = serde_json_upstream::from_reader::<_, serde_json_upstream::Value>(Cursor::new(bytes));
    compare(
        &mut out,
        name,
        "from_reader::<Value>",
        value_t(&rt),
        value_u(&ru),
    );

    // 5. StreamDeserializer: items and byte offsets.
    compare(
        &mut out,
        name,
        "StreamDeserializer",
        stream_t(bytes),
        stream_u(bytes),
    );

    out
}

// ---- the number gate ------------------------------------------------------

fn bits_t<T: FloatBits>(r: Result<T, turbo::Error>) -> String {
    match r {
        Ok(x) => format!("Ok({})", x.bits()),
        Err(e) => err_t(&e),
    }
}

fn bits_u<T: FloatBits>(r: Result<T, serde_json_upstream::Error>) -> String {
    match r {
        Ok(x) => format!("Ok({})", x.bits()),
        Err(e) => err_u(&e),
    }
}

pub trait FloatBits {
    fn bits(&self) -> String;
}
impl FloatBits for f64 {
    fn bits(&self) -> String {
        format!("f64:{:016x}", self.to_bits())
    }
}
impl FloatBits for f32 {
    fn bits(&self) -> String {
        format!("f32:{:08x}", self.to_bits())
    }
}

fn int_t<T: std::fmt::Display>(r: Result<T, turbo::Error>) -> String {
    match r {
        Ok(x) => format!("Ok({x})"),
        Err(e) => err_t(&e),
    }
}

fn int_u<T: std::fmt::Display>(r: Result<T, serde_json_upstream::Error>) -> String {
    match r {
        Ok(x) => format!("Ok({x})"),
        Err(e) => err_u(&e),
    }
}

/// Every numeric target for one token. Float outcomes compare BIT PATTERNS.
pub fn check_number(token: &str) -> Vec<Mismatch> {
    let mut out = Vec::new();
    let doc = format!("number {token:?}");
    compare(
        &mut out,
        &doc,
        "f64 bits",
        bits_t(turbo::from_str::<f64>(token)),
        bits_u(serde_json_upstream::from_str::<f64>(token)),
    );
    compare(
        &mut out,
        &doc,
        "f32 bits",
        bits_t(turbo::from_str::<f32>(token)),
        bits_u(serde_json_upstream::from_str::<f32>(token)),
    );
    compare(
        &mut out,
        &doc,
        "u64",
        int_t(turbo::from_str::<u64>(token)),
        int_u(serde_json_upstream::from_str::<u64>(token)),
    );
    compare(
        &mut out,
        &doc,
        "i64",
        int_t(turbo::from_str::<i64>(token)),
        int_u(serde_json_upstream::from_str::<i64>(token)),
    );
    compare(
        &mut out,
        &doc,
        "u128",
        int_t(turbo::from_str::<u128>(token)),
        int_u(serde_json_upstream::from_str::<u128>(token)),
    );
    compare(
        &mut out,
        &doc,
        "i128",
        int_t(turbo::from_str::<i128>(token)),
        int_u(serde_json_upstream::from_str::<i128>(token)),
    );
    compare(
        &mut out,
        &doc,
        "Value",
        value_t(&turbo::from_str::<turbo::Value>(token)),
        value_u(&serde_json_upstream::from_str::<serde_json_upstream::Value>(token)),
    );
    // The number as an array element exercises the in-structure number path.
    let wrapped = format!("[{token}]");
    compare(
        &mut out,
        &doc,
        "[Value]",
        value_t(&turbo::from_str::<turbo::Value>(&wrapped)),
        value_u(&serde_json_upstream::from_str::<serde_json_upstream::Value>(&wrapped)),
    );
    out
}

/// Maximal runs of number-ish bytes that start with `-` or a digit and are not
/// glued to a preceding identifier byte. Deliberately naive: it also lifts
/// numbers out of string contents, which are just more tokens to compare.
pub fn number_tokens(bytes: &[u8]) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let is_num = |b: u8| b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E');
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let starts = b == b'-' || b.is_ascii_digit();
        let glued = i > 0
            && (bytes[i - 1].is_ascii_alphanumeric()
                || bytes[i - 1] == b'_'
                || bytes[i - 1] == b'.');
        if starts && !glued {
            let mut j = i + 1;
            while j < bytes.len() && is_num(bytes[j]) {
                j += 1;
            }
            if let Ok(s) = std::str::from_utf8(&bytes[i..j]) {
                set.insert(s.to_owned());
            }
            i = j;
        } else {
            i += 1;
        }
    }
    set
}

// ---- edge cases -----------------------------------------------------------

/// Hand-picked documents at the seams: literals, escapes, surrogates, limits,
/// trailing bytes, duplicate keys, empty structures, whitespace.
pub const EDGE_DOCUMENTS: &[&str] = &[
    "",
    " ",
    "\t\n\r ",
    "null",
    "nul",
    "nulll",
    "true",
    "tru",
    "false",
    "fals",
    "[]",
    "{}",
    "[[]]",
    "{\"\":null}",
    "{\"a\":1,\"a\":2}",
    "{\"b\":1,\"a\":2}",
    "[1,]",
    "[,1]",
    "[1 2]",
    "{\"a\" 1}",
    "{\"a\":}",
    "{\"a\":1,}",
    "{a:1}",
    "['a']",
    "[true,false,null]",
    "[1e5,1E5,1e+5,1e-5,1E-0,0e0,-0,-0.0,0.000001]",
    "\"\"",
    "\"a\\\"b\"",
    "\"\\/\\\\\\b\\f\\n\\r\\t\"",
    "\"\\u0000\"",
    "\"\\u001f\"",
    "\"\\u00e9\"",
    "\"h\u{e9}llo\"",
    "\"\\uD83D\\uDE00\"",
    "\"\\ud800\"",
    "\"\\udc00\"",
    "\"\\udc00\\ud800\"",
    "\"\\ud800x\"",
    "\"\\ud800\\u0041\"",
    "\"\\uZZZZ\"",
    "\"\\x41\"",
    "\"\\",
    "\"abc",
    "\"\x01\"",
    "\"\x7f\"",
    "[\"a\",]",
    "{} x",
    "{}\n",
    "[]\r\n",
    "\u{feff}{}",
    "{}{}",
    "1 2",
    "-",
    "01",
    "1.",
    "1.e1",
    ".5",
    "1e",
    "1e+",
    "1.0e+",
    "+1",
    "0x10",
    "Infinity",
    "NaN",
    "-Infinity",
    "18446744073709551615",
    "18446744073709551616",
    "-9223372036854775808",
    "-9223372036854775809",
    "1.7976931348623157e308",
    "1.7976931348623157e309",
    "-1e400",
    "1e-400",
    "4.9e-324",
    "2.2250738585072014e-308",
    "9007199254740993",
    "123456789012345678901234567890",
    "0.1",
    "0.30000000000000004",
    "1.5e-7",
    "{\"a\":{\"b\":{\"c\":[[[]]]}}}",
    "[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]",
    "{\"key with spaces\": \"value\", \"nested\": {\"arr\": [1, 2.5, \"x\", null, true]}}",
];

/// Byte-level cases that are not valid UTF-8 or carry encoded surrogates.
pub const EDGE_BYTES: &[&[u8]] = &[
    b"\xff",
    b"\"\xff\"",
    b"[\"\xc3\x28\"]",
    b"\"\xed\xa0\x80\"",
    b"\"\xf0\x9f\x98\x80\"",
    b"\"\xc0\xaf\"",
    b"{\"\xff\":1}",
    b"\xef\xbb\xbf[]",
];

/// Nesting at and around the recursion limit (128), open and closed.
pub fn depth_documents() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for n in [1usize, 2, 64, 127, 128, 129, 130, 200, 1000] {
        let open = "[".repeat(n);
        out.push((format!("depth {n} open"), open.clone().into_bytes()));
        out.push((
            format!("depth {n} closed"),
            format!("{open}{}", "]".repeat(n)).into_bytes(),
        ));
        let obj = "{\"a\":".repeat(n);
        out.push((
            format!("depth {n} object closed"),
            format!("{obj}1{}", "}".repeat(n)).into_bytes(),
        ));
    }
    out
}

/// Number tokens at the seams of every parse path.
pub const EDGE_NUMBERS: &[&str] = &[
    "0",
    "-0",
    "1",
    "-1",
    "12345678",
    "123456789",
    "1234567890123456789",
    "12345678901234567890",
    "123456789012345678901",
    "18446744073709551615",
    "18446744073709551616",
    "9223372036854775807",
    "9223372036854775808",
    "-9223372036854775808",
    "-9223372036854775809",
    "340282366920938463463374607431768211455",
    "340282366920938463463374607431768211456",
    "-170141183460469231731687303715884105728",
    "-170141183460469231731687303715884105729",
    "0.0",
    "0.5",
    "0.1",
    "0.30000000000000004",
    "1.5",
    "2.5",
    "1e0",
    "1e1",
    "1e-1",
    "1E+2",
    "1.0e10",
    "123.456e-7",
    "1e22",
    "1e23",
    "1e308",
    "1e309",
    "1.7976931348623157e308",
    "1.7976931348623158e308",
    "2.2250738585072014e-308",
    "2.2250738585072011e-308",
    "4.9e-324",
    "5e-324",
    "2.4e-324",
    "1e-400",
    "9007199254740992",
    "9007199254740993",
    "9007199254740993.0",
    "3.4028235e38",
    "3.4028236e38",
    "1.17549435e-38",
    "1.4e-45",
    "0.1e-9999",
    "1.00000000000000000000000000000000000000000000000000001",
    "123456789012345678901234567890.123456789",
    "-1.2345678901234567890e-5",
    "1e",
    "1e+",
    "1.",
    ".1",
    "01",
    "-",
    "--1",
    "1-",
    "1.2.3",
    "1e1e1",
    "1_000",
    "0x10",
];
